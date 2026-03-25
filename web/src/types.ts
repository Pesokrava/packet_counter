/** Shape of a single entry returned by GET /api/stats */
export interface StatEntry {
  protocol: 'tcp' | 'udp' | string
  port: number
  count: number
}

export type SortKey = 'port' | 'protocol' | 'count'
export type SortDir = 'asc' | 'desc'
