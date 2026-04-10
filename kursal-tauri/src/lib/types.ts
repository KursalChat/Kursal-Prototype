export interface ContactResponse {
  userId: string; // hex of UserId bytes
  displayName: string;
  peerId: string;
  knownAddresses: string[];
  verified: boolean;
  profileShared: boolean;
  blocked: boolean;
  createdAt: number;
  avatarBase64?: string | null; // base64 encoded webp string
  avatarBytes?: number[] | null; // Raw byte array from Rust
}

export interface MessageResponse {
  id: string; // hex of MessageId bytes
  contactId: string; // hex of UserId bytes
  direction: "sent" | "received";
  content: string;
  status: "sending" | "delivered" | "failed";
  timestamp: number;
  replyTo: string | null; // hex of MessageId bytes
  reactions?: { emoji: string; userId: string }[];
  edited?: boolean;
}

export interface OtpResponse {
  otp: string;
}

export interface NearbyPeerResponse {
  peerId: string;
  sessionName: string;
}

// Tauri event payloads — mirror what the Rust AppEvent forwarder emits
export type MessageReceivedPayload = MessageResponse;

export interface ConnectionChangedPayload {
  contactId: string;
  status: "connecting" | "relay" | "holepunch" | "direct" | "disconnected";
}

export interface NearbyRequestPayload {
  peerId: string;
  sessionName: string;
}

export interface MessageEditedPayload {
  contactId: string;
  messageId: string;
  newContent: string;
}

export interface MessageDeletedPayload {
  contactId: string;
  messageId: string;
}

export interface ReactionChangedPayload {
  contactId: string;
  messageId: string;
  emoji: string;
  userId?: string;
}
