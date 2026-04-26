import type { Fixture } from './types'

export const emptyFixture: Fixture = {
  name: 'empty',
  projects: [],
  sessions: {},
  sessionDetails: {},
  prefs: {},
  config: {
    notifications: {
      enabled: true,
      soundEnabled: true,
      triggers: [],
    },
    general: {
      launchAtLogin: false,
      showDockIcon: true,
      theme: 'system',
      defaultTab: 'sessions',
      autoExpandAiGroups: false,
    },
  },
  notifications: {
    notifications: [],
    total: 0,
    totalCount: 0,
    unreadCount: 0,
    hasMore: false,
  },
  agentConfigs: [],
  searchResults: [],
}
