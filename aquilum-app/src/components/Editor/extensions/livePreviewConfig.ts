import { Facet } from '@codemirror/state';
import type { LinkDisposition, WikiLinkResolver } from '../../../modules/links';

export type BookCalloutReadRequest = {
  bookFile: string;
  title: string;
  pagesFm?: string;
  bookPagePath?: string;
};

export type LivePreviewConfig = {
  resolveWikiLinks: WikiLinkResolver;
  workspacePath: string | null;
  notePath: () => string;
  onOpenWikiLink: (target: string, disposition: LinkDisposition) => void;
  onOpenExternalUrl: (url: string) => void;
  onReadBook: (request: BookCalloutReadRequest) => void;
};

export const livePreviewConfigFacet = Facet.define<LivePreviewConfig, LivePreviewConfig | null>({
  combine: (values) => values[values.length - 1] ?? null,
});
