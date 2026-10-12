import { describe, expect, it } from 'vitest'
import { clampClip, fileStem, filterRecordings, formatTime } from './library'
import type { Recording } from '../types'

const item = (overrides: Partial<Recording>): Recording => ({ id: 'a', title: '亚海悬城', path: 'a.mp4', sourceId: 'local', account: 'player', createdAt: 1, duration: 15, width: 1920, height: 1080, fps: 60, size: 0, available: true, favoriteMode: 'none', favoritePath: null, categoryIds: [], events: [], metadataStatus: '', thumbnailPath: null, ...overrides })
describe('library filtering', () => {
  it('combines category, account and event searches and preserves original order', () => {
    const recordings = [item({ id: 'one', categoryIds: ['c'], events: [{ id: 'e', kind: 'kill', time: 2, label: '三连击' }] }), item({ id: 'two', account: 'other', categoryIds: ['c'], createdAt: 3 })]
    expect(filterRecordings(recordings, 'category:c', 'player', '三连击', 'newest').map(r => r.id)).toEqual(['one'])
    expect(recordings.map(r => r.id)).toEqual(['one', 'two'])
  })
  it('distinguishes imported sources and both favorite modes', () => {
    const recordings = [item({ id: 'a', favoriteMode: 'index' }), item({ id: 'b', favoriteMode: 'copy', sourceId: 'import' }), item({ id: 'c' })]
    expect(filterRecordings(recordings, 'favorites', '', '', 'newest')).toHaveLength(2)
    expect(filterRecordings(recordings, 'imports', '', '', 'newest', ['import']).map(r => r.id)).toEqual(['b'])
  })
})
describe('media boundaries', () => {
  it('formats hours and handles invalid durations', () => { expect(formatTime(3661.5)).toBe('1:01:01'); expect(formatTime(NaN)).toBe('00:00'); expect(formatTime(-8)).toBe('00:00') })
  it('clamps ranges without accepting reversed or empty clips', () => { expect(clampClip(-2, 12, 10)).toEqual({ start: 0, end: 10, valid: true }); expect(clampClip(5, 3, 10).valid).toBe(false); expect(clampClip(0, Infinity, 10).valid).toBe(false) })
  it('makes Windows-safe export names', () => { expect(fileStem('ACE: A/B?')).toBe('ACE_ A_B_'); expect(fileStem('...')).toBe('moment') })
})
