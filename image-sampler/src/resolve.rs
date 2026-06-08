use crate::locale::{card_json_path, EN};
use crate::plan::{CardIdentity, PlanCard, ResolvedCard};
use crate::progress::{self, StepGuard};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct CardJsonSlim {
    #[serde(default)]
    #[serde(rename = "imagePath")]
    image_path: Option<String>,
    #[serde(default)]
    translations: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolveErrorRow {
    #[serde(flatten)]
    pub card: CardIdentity,
    pub locale: Option<String>,
    pub kind: String,
    pub message: String,
    pub json_path: Option<String>,
}

pub struct ResolveOptions {
    pub plan: PathBuf,
    pub equinox_root: PathBuf,
    pub out_resolved: PathBuf,
    pub out_errors: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolveSummary {
    pub plan_cards: usize,
    pub resolved_cards: usize,
    pub errors: usize,
    pub image_downloads: usize,
    pub jsons_read: usize,
    pub by_set: BTreeMap<String, usize>,
    pub by_locale: BTreeMap<String, usize>,
    pub by_tier: BTreeMap<String, usize>,
}

pub fn run(opts: &ResolveOptions) -> Result<ResolveSummary> {
    const P: &str = "resolve";
    let plan_cards: Vec<PlanCard> = {
        let step = StepGuard::begin(P, "load plan");
        let plan_file = File::open(&opts.plan)
            .with_context(|| format!("open plan {}", opts.plan.display()))?;
        let plan_reader = BufReader::new(plan_file);
        let cards: Vec<PlanCard> = plan_reader
            .lines()
            .enumerate()
            .filter_map(|(line_no, line)| match line {
                Ok(l) if l.trim().is_empty() => None,
                Ok(l) => Some(serde_json::from_str::<PlanCard>(&l).with_context(|| {
                    format!("parse plan card at line {}", line_no + 1)
                })),
                Err(e) => Some(Err(anyhow::Error::from(e))),
            })
            .collect::<Result<_>>()?;
        step.finish(Some(&format!("{} plan cards", cards.len())));
        cards
    };

    if let Some(parent) = opts.out_resolved.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    if let Some(parent) = opts.out_errors.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    let mut resolved_writer = BufWriter::new(
        File::create(&opts.out_resolved)
            .with_context(|| format!("create {}", opts.out_resolved.display()))?,
    );
    let mut errors_writer = BufWriter::new(
        File::create(&opts.out_errors)
            .with_context(|| format!("create {}", opts.out_errors.display()))?,
    );

    let mut json_cache: BTreeMap<String, std::result::Result<CardJsonSlim, String>> =
        BTreeMap::new();
    let mut jsons_read: usize = 0;
    let mut resolved_cards: usize = 0;
    let mut error_count: usize = 0;
    let mut image_downloads: usize = 0;
    let mut by_set: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_locale: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_tier: BTreeMap<String, usize> = BTreeMap::new();

    let resolve_pb = progress::bar(P, "resolve cards", plan_cards.len() as u64);
    for (i, plan) in plan_cards.iter().enumerate() {
        let entry = json_cache.entry(plan.card.reference.clone());
        let parsed = entry.or_insert_with(|| {
            jsons_read += 1;
            let path = card_json_path(
                &opts.equinox_root,
                &plan.card.set,
                &plan.card.faction,
                &plan.card.family_number,
                &plan.card.reference,
            );
            load_card_json(&path).map_err(|e| format!("{e:#}"))
        });
        let card = match parsed {
            Ok(c) => c,
            Err(e) => {
                write_error(
                    &mut errors_writer,
                    &plan.card,
                    None,
                    "json_load_failed",
                    e.clone(),
                    None,
                )?;
                error_count += 1;
                continue;
            }
        };

        let mut resolved_locales: BTreeMap<String, String> = BTreeMap::new();
        let mut card_errors = 0usize;

        for locale in &plan.locales {
            let rel_path_opt = pick_rel_path_from_json(card, locale);
            let Some(rel_path) = rel_path_opt else {
                write_error(
                    &mut errors_writer,
                    &plan.card,
                    Some(locale.clone()),
                    "missing_locale_image",
                    format!("no image for locale {locale}"),
                    Some(card_json_path(
                        &opts.equinox_root,
                        &plan.card.set,
                        &plan.card.faction,
                        &plan.card.family_number,
                        &plan.card.reference,
                    )),
                )?;
                error_count += 1;
                card_errors += 1;
                if locale == EN {
                    break;
                }
                continue;
            };

            resolved_locales.insert(locale.clone(), rel_path);
            *by_locale.entry(locale.clone()).or_insert(0) += 1;
            image_downloads += 1;
        }

        if !resolved_locales.contains_key(EN) {
            if card_errors == 0 {
                write_error(
                    &mut errors_writer,
                    &plan.card,
                    Some(EN.to_string()),
                    "missing_required_en",
                    "required en_US image missing".to_string(),
                    Some(card_json_path(
                        &opts.equinox_root,
                        &plan.card.set,
                        &plan.card.faction,
                        &plan.card.family_number,
                        &plan.card.reference,
                    )),
                )?;
                error_count += 1;
            }
            continue;
        }

        let resolved = ResolvedCard {
            card: plan.card.clone(),
            locale_tier: plan.locale_tier,
            shape_floor: plan.shape_floor,
            locales: resolved_locales,
        };
        serde_json::to_writer(&mut resolved_writer, &resolved)?;
        resolved_writer.write_all(b"\n")?;
        resolved_cards += 1;
        *by_set.entry(plan.card.set.clone()).or_insert(0) += 1;
        *by_tier
            .entry(plan.locale_tier.as_str().to_string())
            .or_insert(0) += 1;
        resolve_pb.set_position((i + 1) as u64);
    }
    progress::finish_bar(
        resolve_pb,
        format!("{resolved_cards} resolved, {error_count} errors"),
    );

    resolved_writer.flush()?;
    errors_writer.flush()?;

    Ok(ResolveSummary {
        plan_cards: plan_cards.len(),
        resolved_cards,
        errors: error_count,
        image_downloads,
        jsons_read,
        by_set,
        by_locale,
        by_tier,
    })
}

fn load_card_json(path: &Path) -> Result<CardJsonSlim> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let card: CardJsonSlim = serde_json::from_str(&text)
        .with_context(|| format!("parse JSON {}", path.display()))?;
    Ok(card)
}

fn pick_rel_path_from_json(card: &CardJsonSlim, locale: &str) -> Option<String> {
    if let Some(t) = card.translations.get(locale) {
        if let Some(img) = t
            .get("image")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(img.to_string());
        }
    }
    if locale == EN {
        if let Some(p) = card.image_path.as_deref().filter(|s| !s.is_empty()) {
            return Some(crate::url::rel_path_from_dev_url(p).to_string());
        }
    }
    None
}

fn write_error<W: Write>(
    writer: &mut W,
    card: &CardIdentity,
    locale: Option<String>,
    kind: &str,
    message: String,
    json_path: Option<PathBuf>,
) -> Result<()> {
    let err = ResolveErrorRow {
        card: card.clone(),
        locale,
        kind: kind.to_string(),
        message,
        json_path: json_path.map(|p| p.display().to_string()),
    };
    serde_json::to_writer(&mut *writer, &err)?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn print_summary(summary: &ResolveSummary) {
    println!("== resolve-urls ==");
    println!(
        "  plan_cards={}, resolved_cards={}, image_downloads={}, errors={}, jsons_read={}",
        summary.plan_cards,
        summary.resolved_cards,
        summary.image_downloads,
        summary.errors,
        summary.jsons_read
    );
    println!();
    println!("  by tier:");
    for (tier, count) in &summary.by_tier {
        println!("    {:<10} {}", tier, count);
    }
    println!();
    println!("  by set:");
    for (set, count) in &summary.by_set {
        println!("    {:<10} {}", set, count);
    }
    println!();
    println!("  by locale:");
    for (locale, count) in &summary.by_locale {
        println!("    {:<8} {}", locale, count);
    }
}
