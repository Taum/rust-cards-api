import {

  useEffect,

  useId,

  useLayoutEffect,

  useMemo,

  useRef,

  useState,

  type KeyboardEvent,

  type RefObject,

} from 'react';

import { createPortal } from 'react-dom';

import { localeText, type CardLocale } from '../locale';

import type { EffectCatalogItem } from '../types';



const MAX_FILTERED_SUGGESTIONS = 40;



export type EffectMenuAlign = 'start' | 'end';



type EffectIdComboboxProps = {

  label: string;

  value: string;

  onChange: (value: string) => void;

  options: EffectCatalogItem[];

  locale: CardLocale;

  disabled?: boolean;

  placeholder?: string;

  /** Anchor dropdown to input start (default) or end — use end for right-column fields. */

  menuAlign?: EffectMenuAlign;

  /** When non-null, restrict suggestions to these idGds (live narrowing). */

  availableIds?: number[] | null;

  /** True while a narrowing request for this box is in flight. */

  narrowing?: boolean;

  /** Fired when this box gains (true) or loses (false) focus. */

  onFocusChange?: (focused: boolean) => void;

};



type MenuPosition = {

  top: number;

  left?: number;

  right?: number;

  minWidth: number;

};



function useMenuPosition(

  open: boolean,

  anchorRef: RefObject<HTMLElement | null>,

  align: EffectMenuAlign,

): MenuPosition | null {

  const [position, setPosition] = useState<MenuPosition | null>(null);



  useLayoutEffect(() => {

    if (!open || !anchorRef.current) {

      setPosition(null);

      return;

    }



    const update = () => {

      const el = anchorRef.current;

      if (!el) {

        return;

      }

      const rect = el.getBoundingClientRect();

      setPosition({

        top: rect.bottom + 4,

        ...(align === 'end'

          ? { right: window.innerWidth - rect.right }

          : { left: rect.left }),

        minWidth: rect.width,

      });

    };



    update();

    window.addEventListener('resize', update);

    window.addEventListener('scroll', update, true);

    return () => {

      window.removeEventListener('resize', update);

      window.removeEventListener('scroll', update, true);

    };

  }, [open, align, anchorRef]);



  return position;

}



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



function appendSelection(value: string, idGd: number): string {

  const id = String(idGd);

  const trimmed = value.trim();

  if (!trimmed) {

    return id;

  }

  if (trimmed.endsWith(',')) {

    return `${trimmed}${id}`;

  }

  return `${trimmed},${id}`;

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

  menuAlign = 'start',

  availableIds = null,

  narrowing = false,

  onFocusChange,

}: EffectIdComboboxProps) {

  const listId = useId();

  const anchorRef = useRef<HTMLDivElement>(null);

  const inputRef = useRef<HTMLInputElement>(null);

  const [open, setOpen] = useState(false);

  const [highlighted, setHighlighted] = useState(0);

  /** False on focus: show full list and append on pick. True after user edits input. */

  const [isFiltering, setIsFiltering] = useState(false);



  const token = activeToken(value);



  const availableSet = useMemo(

    () => (availableIds == null ? null : new Set(availableIds)),

    [availableIds],

  );



  const narrowedOptions = useMemo(() => {

    if (!availableSet) {

      return options;

    }

    return options.filter((item) => availableSet.has(item.idGd));

  }, [options, availableSet]);



  const suggestions = useMemo(() => {

    if (!isFiltering) {

      return narrowedOptions;

    }

    return narrowedOptions

      .filter((item) => matchesOption(item, token, locale))

      .slice(0, MAX_FILTERED_SUGGESTIONS);

  }, [narrowedOptions, token, locale, isFiltering]);



  const narrowingActive = availableSet != null;

  const noneAvailable = narrowingActive && !narrowing && narrowedOptions.length === 0;



  useEffect(() => {

    setHighlighted(0);

  }, [token, suggestions.length, isFiltering]);



  const showList = open && !disabled && suggestions.length > 0;

  const menuPosition = useMenuPosition(showList, anchorRef, menuAlign);

  const showClear = !disabled && value.trim() !== '';



  const clearValue = () => {

    onChange('');

    setIsFiltering(false);

    setOpen(false);

    inputRef.current?.focus();

  };



  const selectItem = (item: EffectCatalogItem) => {

    if (isFiltering) {

      onChange(applySelection(value, item.idGd));

      setOpen(false);

    } else {

      onChange(appendSelection(value, item.idGd));

      setOpen(true);

    }

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



  const listbox =

    showList && menuPosition ? (

      <ul

        id={listId}

        role="listbox"

        style={{

          position: 'fixed',

          top: menuPosition.top,

          left: menuPosition.left,

          right: menuPosition.right,

          minWidth: menuPosition.minWidth,

        }}

        className="z-50 max-h-72 w-max max-w-[min(32rem,calc(100vw-1rem))] overflow-y-auto overflow-x-auto rounded border border-slate-600 bg-slate-900 py-1 shadow-xl"

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

              className={`cursor-pointer px-3 py-2 text-sm leading-snug whitespace-normal ${

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

              <span className="text-slate-300 break-words">

                {text || '(no text)'}

              </span>

            </li>

          );

        })}

      </ul>

    ) : null;



  return (

    <div className="relative block text-xs text-slate-400">

      <span className="mb-1 block">

        {label}

        {narrowingActive && (

          <span

            className={`ml-1.5 ${

              noneAvailable ? 'text-amber-400' : 'text-slate-500'

            }`}

          >

            {narrowing

              ? '· narrowing…'

              : noneAvailable

                ? '· none possible'

                : `· ${narrowedOptions.length} possible`}

          </span>

        )}

      </span>

      <div ref={anchorRef} className="relative">

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

            setIsFiltering(true);

            onChange(e.target.value);

            setOpen(true);

          }}

          onFocus={() => {

            setIsFiltering(false);

            setOpen(true);

            onFocusChange?.(true);

          }}

          onBlur={() => {

            onFocusChange?.(false);

            window.setTimeout(() => setOpen(false), 150);

          }}

          onKeyDown={onKeyDown}

          className={`mt-0 w-full rounded border border-slate-600 bg-slate-950 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 ${

            showClear ? 'pl-2 pr-7' : 'px-2'

          }`}

        />

        {showClear && (

          <button

            type="button"

            tabIndex={-1}

            aria-label={`Clear ${label}`}

            onMouseDown={(e) => e.preventDefault()}

            onClick={clearValue}

            className="absolute right-1 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-slate-500 hover:bg-slate-800 hover:text-slate-200"

          >

            <span aria-hidden className="text-sm leading-none">

              ×

            </span>

          </button>

        )}

      </div>

      {listbox && createPortal(listbox, document.body)}

    </div>

  );

}


