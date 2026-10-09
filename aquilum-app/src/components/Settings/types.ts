import type { AppConfig } from '../../modules/settings';

export type SettingsSectionId = 'ui' | 'editor' | 'reader' | 'search' | 'templates' | 'files' | 'analysis' | 'mcp' | 'history' | 'trash' | 'system' | 'shortcuts';

export interface SettingsSectionProps {
  config: AppConfig;
  onChange: (next: AppConfig) => void;
}
