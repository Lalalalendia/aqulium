import type { WorkspaceItem } from '../../modules/documents/fileGateway';
import type { FileMenuActions } from './fileActionItems';
import { comparablePath } from '../../modules/paths';

export interface VisibleFileRow {
  item: WorkspaceItem;
  comparableId: string;
  depth: number;
  expanded: boolean;
  loading: boolean;
  guideDepths: string;
}

export interface FileTreeActions extends FileMenuActions {
  selectFile: (path: string, isMulti: boolean, isRange: boolean) => void;
  openInNewTab: (path: string) => void;
  focusRow: (path: string) => void;
  toggleFolder: (id: string) => void;
  prefetchFolder: (id: string) => void;
  commitRename: (path: string, nextName: string) => void;
  cancelRename: () => void;
}

export function parseGuideDepths(guideDepths: string): number[] {
  return guideDepths === '' ? [] : guideDepths.split(',').map(Number);
}

export function buildVisibleFileRows(
  roots: readonly WorkspaceItem[],
  directories: ReadonlyMap<string, WorkspaceItem[]>,
  expandedFolders: ReadonlySet<string>,
  loadingDirectories: ReadonlySet<string>,
): VisibleFileRow[] {
  const rows: VisibleFileRow[] = [];

  const visit = (items: readonly WorkspaceItem[], depth: number, continuingGuides: number[]) => {
    const guideDepths = continuingGuides.join(',');

    items.forEach((item, index) => {
      const isFolder = item.type === 'folder';
      const expanded = isFolder && expandedFolders.has(item.id);
      const isLast = index === items.length - 1;

      rows.push({
        item,
        comparableId: comparablePath(item.id),
        depth,
        expanded,
        loading: isFolder && loadingDirectories.has(item.id),
        guideDepths,
      });

      if (expanded) {
        const children = directories.get(item.id);
        if (children && children.length > 0) {
          let guidesForChildren = [...continuingGuides];
          if (isLast) {
            guidesForChildren = guidesForChildren.filter(g => g !== depth - 1);
          }
          guidesForChildren.push(depth);
          visit(children, depth + 1, guidesForChildren);
        }
      }
    });
  };

  visit(roots, 0, []);
  return rows;
}

export function unloadedExpandedFolders(
  rows: readonly VisibleFileRow[],
  directories: ReadonlyMap<string, WorkspaceItem[]>,
): string[] {
  return rows
    .filter((row) => row.expanded && !directories.has(row.item.id))
    .map((row) => row.item.id);
}
