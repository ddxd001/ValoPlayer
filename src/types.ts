export type FavoriteMode = 'none' | 'index' | 'copy'
export interface MomentEvent { id: string; kind: 'kill' | 'death' | 'assist' | 'round' | 'unknown'; time: number; label: string }
export interface Recording {
  id: string; title: string; path: string; sourceId: string; account: string; createdAt: number
  duration: number; width: number; height: number; fps: number; size: number; available: boolean
  favoriteMode: FavoriteMode; favoritePath: string | null; categoryIds: string[]; events: MomentEvent[]
  metadataStatus: string; thumbnailPath: string | null
}
export interface Source { id: string; name: string; path: string; kind: 'wegame' | 'import'; account: string; status: string }
export interface Category { id: string; name: string }
export interface Settings { ffmpegPath: string; continuousPlay: boolean; autoScan: boolean }
export interface Snapshot { recordings: Recording[]; sources: Source[]; categories: Category[]; settings: Settings; diagnostics: string[] }
export interface Environment { ffmpegAvailable: boolean; ffmpegPath: string; dataDir: string }
export type LibraryView = 'all' | 'favorites' | 'imports' | `category:${string}` | `source:${string}`
export type SortOrder = 'newest' | 'oldest' | 'duration' | 'title'
export type KillCountFilter = 'all' | '0' | '1' | '2' | '3' | '4' | '5' | '6+'
