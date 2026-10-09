import { memo, useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { FileText } from 'lucide';
import type { WorkspaceItem } from '../../modules/documents/fileGateway';
import { Button } from '../Common/Button';
import { DeleteNotesDialog } from '../Common/DeleteNotesDialog';
import { EmptyState } from '../Common/EmptyState';
import { FileTree } from './FileTree';
import { t } from '../../i18n';
import { SidebarFooter } from './SidebarFooter';
import { buildVisibleFileRows, unloadedExpandedFolders, type FileTreeActions } from './fileTreeModel';
import type { LinkDisposition } from '../../modules/links';
import { useExpandedFolders } from '../../modules/workspace/uiPersist';
import { moveIntoFolder } from './moveIntoFolder';
import { useFileTreeDrag } from './useFileDragAndDrop';
import { useFileSelection } from './useFileSelection';
import { useFileTreeActions } from './useFileTreeActions';
import { useStableCallback } from '../../hooks/useStableCallback';
import './Sidebar.css';

interface SidebarProps {
  activeFile: string | null;
  files: WorkspaceItem[];
  directories: ReadonlyMap<string, WorkspaceItem[]>;
  loadingDirectories: ReadonlySet<string>;
  workspacePath: string | null;
  workspaceName: string;
  onFileSelect: (path: string, options?: { disposition?: LinkDisposition }) => void;
  onCreateNote: () => void | Promise<void>;
  onLoadDirectory: (path: string) => Promise<boolean>;
  isOpen: boolean;
  onOpenSettings: () => void;
  onOpenWorkspaces: () => void;
  onPatchFileInTree?: (path: string, patch: { id?: string; name?: string }) => void;
}

export const Sidebar = memo(function Sidebar({
  activeFile,
  files,
  directories,
  loadingDirectories,
  workspacePath,
  workspaceName,
  onFileSelect,
  onCreateNote,
  onLoadDirectory,
  isOpen,
  onOpenSettings,
  onOpenWorkspaces,
  onPatchFileInTree,
}: SidebarProps) {
  const { expandedFolders, setExpandedFolders, followFolder } = useExpandedFolders(workspacePath);
  const contentRef = useRef<HTMLDivElement>(null);
  const pendingScrollTopRef = useRef<number | null>(null);
  const treeReady = Boolean(workspacePath && directories.has(workspacePath));
  const rows = useMemo(() => {
    if (!treeReady) return [];
    return buildVisibleFileRows(files, directories, expandedFolders, loadingDirectories);
  }, [directories, expandedFolders, files, loadingDirectories, treeReady]);

  const { selectedFiles, rowActions: selectionActions, targetsFor, clearSelection } = useFileSelection(
    activeFile,
    rows,
    onFileSelect,
  );
  const fileOps = useFileTreeActions({
    workspacePath,
    targetsFor,
    clearSelection,
    onPatchFileInTree,
    onPathMoved: followFolder,
  });

  const moveFiles = useStableCallback(async (sourceId: string, targetId: string) => {
    let anyMoved = false;
    for (const fileId of targetsFor(sourceId)) {
      const moved = await moveIntoFolder(fileId, targetId);
      if (!moved) continue;
      anyMoved = true;
      followFolder(fileId, moved);
    }

    if (!anyMoved) return;
    setExpandedFolders((prev) => (
      prev.has(targetId) ? prev : new Set(prev).add(targetId)
    ));
    void onLoadDirectory(targetId);
  });

  const toggleFolder = useStableCallback((id: string) => {
    pendingScrollTopRef.current = contentRef.current?.scrollTop ?? null;
    setExpandedFolders((current) => {
      const next = new Set(current);
      if (!current.has(id)) next.add(id);
      else next.delete(id);
      return next;
    });
  });

  const loadRevealedFolders = useStableCallback(() => {
    for (const id of unloadedExpandedFolders(rows, directories)) void onLoadDirectory(id);
  });

  useEffect(loadRevealedFolders, [directories, expandedFolders, loadRevealedFolders]);

  const prefetchFolder = useStableCallback((id: string) => {
    if (!directories.has(id)) void onLoadDirectory(id);
  });

  useFileTreeDrag(contentRef, targetsFor, moveFiles);

  const treeActions = useMemo<FileTreeActions>(() => ({
    ...selectionActions,
    ...fileOps.rowActions,
    toggleFolder,
    prefetchFolder,
  }), [fileOps.rowActions, prefetchFolder, selectionActions, toggleFolder]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const item = content?.querySelector<HTMLElement>('[data-file-active="true"]');
    if (!item || !content) return;

    const contentRect = content.getBoundingClientRect();
    const itemRect = item.getBoundingClientRect();
    if (itemRect.top < contentRect.top) {
      content.scrollTop -= contentRect.top - itemRect.top;
    } else if (itemRect.bottom > contentRect.bottom) {
      content.scrollTop += itemRect.bottom - contentRect.bottom;
    }
  }, [activeFile]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const scrollTop = pendingScrollTopRef.current;
    if (!content || scrollTop === null) return;
    content.scrollTop = scrollTop;
    pendingScrollTopRef.current = null;
  }, [expandedFolders]);

  const { deleteTargets } = fileOps;

  return (
    <aside className={`q-collapsible-panel q-sidebar ${isOpen ? '' : 'is-collapsed'}`} aria-hidden={!isOpen}>
      <div className="q-panel-header" data-tauri-drag-region aria-hidden="true">
      </div>
      <div ref={contentRef} className="q-sidebar-content">
        {!treeReady ? null : rows.length === 0 ? (
          <EmptyState icon={FileText} title={t('fileTree.empty')} compact>
            <Button size="s" onClick={() => void onCreateNote()}>
              {t('editor.createNote')}
            </Button>
          </EmptyState>
        ) : (
          <FileTree
            rows={rows}
            activeFile={activeFile}
            selectedFiles={selectedFiles}
            renamingPath={fileOps.renamingPath}
            actions={treeActions}
          />
        )}
      </div>

      <DeleteNotesDialog
        targets={deleteTargets}
        pending={fileOps.isDeleting}
        onCancel={fileOps.dismissDelete}
        onConfirm={fileOps.confirmDelete}
      />

      <SidebarFooter
        workspaceName={workspaceName}
        workspacePath={workspacePath}
        onOpenSettings={onOpenSettings}
        onOpenWorkspaces={onOpenWorkspaces}
      />
    </aside>
  );
});
