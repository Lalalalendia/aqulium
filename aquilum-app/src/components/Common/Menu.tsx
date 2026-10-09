import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { Check, type IconNode } from 'lucide';
import { Icon } from './Icon';
import './Menu.css';

export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  checked?: boolean;
  icon?: IconNode;
  onSelect: () => void;
}

export interface MenuPosition {
  top: number;
  left: number;
  width?: number;
}

interface MenuProps {
  id?: string;
  open: boolean;
  position: MenuPosition | null;
  items: MenuItem[];
  onClose: () => void;
  role?: 'listbox' | 'menu';
  ariaLabel?: string;
  showCheck?: boolean;
  excludeRef?: RefObject<HTMLElement | null>;
}

const VIEWPORT_MARGIN_PX = 8;

export function Menu({
  id,
  open,
  position,
  items,
  onClose,
  role = 'menu',
  ariaLabel,
  showCheck = false,
  excludeRef,
}: MenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!open || !position || !menu) return;
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) {
      menu.style.left = `${Math.max(VIEWPORT_MARGIN_PX, position.left - rect.width)}px`;
    }
    if (rect.bottom > window.innerHeight) {
      menu.style.top = `${Math.max(VIEWPORT_MARGIN_PX, position.top - rect.height)}px`;
    }
  }, [open, position]);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (menuRef.current?.contains(target) || excludeRef?.current?.contains(target)) return;
      onClose();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      onClose();
    };

    const handleReposition = () => onClose();

    window.addEventListener('mousedown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('resize', handleReposition);
    window.addEventListener('scroll', handleReposition, true);

    return () => {
      window.removeEventListener('mousedown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('resize', handleReposition);
      window.removeEventListener('scroll', handleReposition, true);
    };
  }, [excludeRef, onClose, open]);

  if (!open || !position) return null;

  return createPortal(
    <div
      ref={menuRef}
      id={id}
      role={role}
      aria-label={ariaLabel}
      className="q-menu"
      style={{
        top: position.top,
        left: position.left,
        width: position.width,
      }}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          role={role === 'listbox' ? 'option' : 'menuitem'}
          aria-selected={role === 'listbox' ? Boolean(item.checked) : undefined}
          disabled={item.disabled}
          className="q-menu__item"
          onClick={() => {
            if (item.disabled) return;
            item.onSelect();
            onClose();
          }}
        >
          {item.icon && (
            <span className="q-menu__item-icon" aria-hidden="true">
              <Icon icon={item.icon} strokeWidth={1.2} />
            </span>
          )}
          <span className="q-menu__item-label">{item.label}</span>
          {showCheck && item.checked && (
            <Icon icon={Check} className="q-menu__item-check" />
          )}
        </button>
      ))}
    </div>,
    document.body,
  );
}
