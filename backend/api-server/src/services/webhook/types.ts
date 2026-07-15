/**
 * Webhook Service Types
 *
 * Rule 00 (No Mocks): All webhook deliveries are tracked and logged.
 * Fail-Open: Webhook failures do not block system execution.
 */

export type WebhookPlatform = 'slack' | 'discord';

export type WebhookEventType =
  | 'opportunity.detected'
  | 'opportunity.scored'
  | 'execution.submitted'
  | 'execution.included'
  | 'execution.confirmed'
  | 'execution.failed'
  | 'gate.triggered'
  | 'system.alert';

export interface WebhookConfig {
  id: string;
  name: string;
  url: string;
  platform: WebhookPlatform;
  events: WebhookEventType[];
  secret?: string;
  retryMaxAttempts: number;
  retryBackoffMs: number;
  enabled: boolean;
  createdAt: Date;
}

export interface WebhookDelivery {
  id: string;
  configId: string;
  eventType: WebhookEventType;
  payload: Record<string, unknown>;
  status: 'pending' | 'delivered' | 'failed' | 'dlq';
  attempts: number;
  responseStatus?: number;
  responseBody?: string;
  deliveredAt?: Date;
  errorMessage?: string;
  dedupHash: string;
  createdAt: Date;
}

export interface WebhookPayload {
  event: WebhookEventType;
  timestamp: string;
  data: Record<string, unknown>;
  signature?: string;
}

export interface SlackPayload {
  text?: string;
  blocks?: SlackBlock[];
  attachments?: SlackAttachment[];
  unfurl_links?: boolean;
}

export interface SlackBlock {
  type: 'header' | 'section' | 'divider' | 'context';
  text?: SlackText;
  fields?: SlackText[];
  elements?: SlackElement[];
}

export interface SlackText {
  type: 'mrkdwn' | 'plain_text';
  text: string;
  emoji?: boolean;
}

export interface SlackElement {
  type: 'mrkdwn' | 'image';
  text?: string;
  image_url?: string;
  alt_text?: string;
}

export interface SlackAttachment {
  color: string;
  title: string;
  text: string;
  fields?: { title: string; value: string; short: boolean }[];
  footer?: string;
  ts?: number;
}

export interface DiscordPayload {
  content?: string;
  embeds?: DiscordEmbed[];
  username?: string;
  avatar_url?: string;
}

export interface DiscordEmbed {
  title: string;
  description?: string;
  color: number;
  fields?: { name: string; value: string; inline?: boolean }[];
  footer?: { text: string };
  timestamp?: string;
}

export interface RetryConfig {
  maxAttempts: number;
  backoffMs: number;
  maxBackoffMs: number;
  jitter: boolean;
}
