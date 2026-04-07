import { invoke } from '@tauri-apps/api/core';
import type { NearbyPeerResponse } from '$lib/types';

// Returns the session name string (e.g. "Purple Raccoon")
export const startNearby = (): Promise<string> =>
  invoke('start_nearby');

export const stopNearby = (): Promise<void> =>
  invoke('stop_nearby');

export const getNearbyPeers = (): Promise<NearbyPeerResponse[]> =>
  invoke('get_nearby_peers');

export const connectNearby = (peerId: string): Promise<void> =>
  invoke('connect_nearby', { peerId });

export const acceptNearby = (peerId: string): Promise<void> =>
  invoke('accept_nearby', { peerId });

export const declineNearby = (peerId: string): Promise<void> =>
  invoke('decline_nearby', { peerId });
