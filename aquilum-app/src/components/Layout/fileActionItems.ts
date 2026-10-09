import { t } from '../../i18n';
import type { MenuItem } from '../Common/Menu';

export interface FileMenuActions {
  startRename: (path: string) => void;
  duplicateFile: (path: string) => void;
  requestDelete: (path: string) => void;
}

export function renameItem(onSelect: () => void): MenuItem {
  return { id: 'rename', label: t('common.rename'), onSelect };
}

export function fileActionItems(actions: FileMenuActions, path: string): MenuItem[] {
  return [
    renameItem(() => actions.startRename(path)),
    { id: 'duplicate', label: t('common.duplicate'), onSelect: () => actions.duplicateFile(path) },
    { id: 'delete', label: t('common.delete'), onSelect: () => actions.requestDelete(path) },
  ];
}
