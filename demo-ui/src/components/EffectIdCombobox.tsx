import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';
import { localeText, type CardLocale } from '../locale';
import type { EffectCatalogItem } from '../types';

const MAX_SUGGESTIONS = 40;
const LABEL_MAX_LEN = 72;

type EffectIdComboboxProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: EffectCatalogItem[];
  locale: CardLocale;
  disabled?: boolean;
  placeholder?: string;
};

function activeToken(value: string): string {
  const parts = value.split(',');
  return (parts[parts.length - 1] ?? '').trim();
}

function prefixBeforeActiveToken(value: string): string {
  const idx = value.lastIndexOf(',');
  if (idx === -1) {
    return '';
  }
  return value.slice(0, idx + 1);
}

function applySelection(value: string, idGd: number): string {
  const prefix = prefixBeforeActiveToken(value);
  const id = String(idGd);
  return prefix ? `${prefix}${id}` : id;
}

function truncate(text: string, max: number): string {
  if (text.length <= max) {
    return text;
  }
  return `${text.slice(0, max - 1)}…`;
}

function matchesOption(
  item: EffectCatalogItem,
  token: string,
  locale: CardLocale,
): boolean {
  if (!token) {
    return true;
  }
  const lower = token.toLowerCase();
  if (String(item.idGd).includes(lower)) {
    return true;
  }
  const label = localeText(item.text, locale).toLowerCase();
  return label.includes(lower);
}

export function EffectIdCombobox({
  label,
  value,
  onChange,
  options,
  locale,
  disabled = false,
  placeholder,
}: EffectIdComboboxProps) {
  const listId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(0);

  const token = activeToken(value);

  const suggestions = useMemo(() => {
    const filtered = options.filter((item) => matchesOption(item, token, locale));
    return filtered.slice(0, MAX_SUGGESTIONS);
  }, [options, token, locale]);

  useEffect(() => {
    setHighlighted(0);
  }, [token, suggestions.length]);

  const showList = open && !disabled && suggestions.length > 0;

  const selectItem = (item: EffectCatalogItem) => {
    onChange(applySelection(value, item.idGd));
    setOpen(false);
    inputRef.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      setOpen(true);
      e.preventDefault();
      return;
    }

    if (!showList) {
      if (e.key === 'Escape') {
        setOpen(false);
      }
      return;
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setHighlighted((i) => Math.min(i + 1, suggestions.length - 1));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setHighlighted((i) => Math.max(i - 1, 0));
        break;
      case 'Enter':
        e.preventDefault();
        if (suggestions[highlighted]) {
          selectItem(suggestions[highlighted]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        setOpen(false);
        break;
      case 'Tab':
        setOpen(false);
        break;
      default:
        break;
    }
  };

  return (
    <div className="relative block text-xs text-slate-400">
      <span className="mb-1 block">{label}</span>
      <input
        ref={inputRef}
        type="text"
        role="combobox"
        aria-expanded={showList}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={
          showList ? `${listId}-option-${highlighted}` : undefined
        }
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => {
          window.setTimeout(() => setOpen(false), 150);
        }}
        onKeyDown={onKeyDown}
        className="mt-0 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
      />
      {showList && (
        <ul
          id={listId}
          role="listbox"
          className="absolute z-20 mt-1 max-h-56 w-full overflow-y-auto rounded border border-slate-600 bg-slate-900 py-1 shadow-lg"
        >
          {suggestions.map((item, index) => {
            const text = localeText(item.text, locale);
            const active = index === highlighted;
            return (
              <li
                key={item.idGd}
                id={`${listId}-option-${index}`}
                role="option"
                aria-selected={active}
                className={`cursor-pointer px-2 py-1.5 text-sm ${
                  active
                    ? 'bg-sky-700/40 text-slate-50'
                    : 'text-slate-200 hover:bg-slate-800'
                }`}
                onMouseDown={(e) => e.preventDefault()}
                onMouseEnter={() => setHighlighted(index)}
                onClick={() => selectItem(item)}
              >
                <span className="font-medium text-sky-300">{item.idGd}</span>
                <span className="text-slate-400"> — </span>
                <span className="text-slate-300">
                  {truncate(text, LABEL_MAX_LEN) || '(no text)'}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
