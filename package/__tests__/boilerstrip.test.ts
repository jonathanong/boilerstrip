import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect } from 'vitest'
import { learn, learnSync, convert, convertSync, type Removals } from '../index'

const FIXTURES = join(__dirname, '../../fixtures')

function readBuf(path: string): Buffer {
  return readFileSync(join(FIXTURES, path))
}

function readStr(path: string): string {
  return readFileSync(join(FIXTURES, path), 'utf-8')
}

const siteABufs = ['page1.html', 'page2.html', 'page3.html'].map((f) =>
  readBuf(`learn/site_a/${f}`),
)
const siteAStrs = ['page1.html', 'page2.html', 'page3.html'].map((f) =>
  readStr(`learn/site_a/${f}`),
)

describe('learn (async)', () => {
  it('detects shared selectors from site_a (Buffer input)', async () => {
    const removals = await learn(siteABufs)
    expect(removals.cssSelectorsToRemove).toContain('.site-nav')
    expect(removals.cssSelectorsToRemove).toContain('#site-footer')
  })

  it('accepts string input', async () => {
    const removals = await learn(siteAStrs)
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
    expect(Array.isArray(removals.htmlToRemove)).toBe(true)
  })

  it('accepts mixed Buffer and string input', async () => {
    const mixed = [siteABufs[0], siteAStrs[1], siteABufs[2]]
    const removals = await learn(mixed)
    expect(removals.cssSelectorsToRemove).toContain('.site-nav')
  })

  it('rejects when fewer than 2 pages are provided', async () => {
    await expect(learn([readBuf('learn/site_a/page1.html')])).rejects.toThrow()
  })
})

describe('learnSync', () => {
  it('detects shared selectors from site_a (string input)', () => {
    const removals = learnSync(siteAStrs)
    expect(removals.cssSelectorsToRemove).toContain('.site-nav')
    expect(removals.cssSelectorsToRemove).toContain('#site-footer')
  })

  it('accepts Buffer input', () => {
    const removals = learnSync(siteABufs)
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
  })

  it('throws when fewer than 2 pages are provided', () => {
    expect(() => learnSync(['<html></html>'])).toThrow()
  })
})

describe('convert (async)', () => {
  it('converts a batch of Buffers to Buffers', async () => {
    const buf = readBuf('convert/basic_article.html')
    const results = await convert([buf])
    expect(results).toHaveLength(1)
    expect(Buffer.isBuffer(results[0])).toBe(true)
    expect(results[0].toString()).toContain('Getting Started')
  })

  it('returns empty array for empty input', async () => {
    const results = await convert([])
    expect(results).toHaveLength(0)
  })

  it('processes a batch of multiple pages', async () => {
    const html1 = '<html><body><main><h1>Page One</h1></main></body></html>'
    const html2 = '<html><body><main><h1>Page Two</h1></main></body></html>'
    const results = await convert([html1, html2])
    expect(results).toHaveLength(2)
    expect(results[0].toString()).toContain('Page One')
    expect(results[1].toString()).toContain('Page Two')
  })

  it('strips boilerplate when removals provided', async () => {
    const pages = [
      '<html><body><nav class="nav">Menu</nav><main>Page 1 content</main></body></html>',
      '<html><body><nav class="nav">Menu</nav><main>Page 2 content</main></body></html>',
    ]
    const removals: Removals = await learn(pages)
    const html = '<html><body><nav class="nav">Menu</nav><main>Article content</main></body></html>'
    const [result] = await convert([html], removals)
    expect(result.toString()).not.toContain('Menu')
    expect(result.toString()).toContain('Article content')
  })

  it('accepts mixed Buffer and string input', async () => {
    const buf = readBuf('convert/basic_article.html')
    const str = '<html><body><main><p>hello</p></main></body></html>'
    const results = await convert([buf, str])
    expect(results).toHaveLength(2)
    expect(results[0].toString()).toContain('Getting Started')
    expect(results[1].toString()).toContain('hello')
  })
})

describe('convertSync', () => {
  it('converts a batch of strings, returns Buffers', () => {
    const html = readStr('convert/basic_article.html')
    const results = convertSync([html])
    expect(results).toHaveLength(1)
    expect(Buffer.isBuffer(results[0])).toBe(true)
    expect(results[0].toString()).toContain('Getting Started')
  })

  it('returns empty array for empty input', () => {
    expect(convertSync([])).toHaveLength(0)
  })

  it('strips boilerplate when removals provided', () => {
    const pages = [
      '<html><body><nav class="nav">Menu</nav><main>Page 1</main></body></html>',
      '<html><body><nav class="nav">Menu</nav><main>Page 2</main></body></html>',
    ]
    const removals: Removals = learnSync(pages)
    const html = '<html><body><nav class="nav">Menu</nav><main>Article</main></body></html>'
    const [result] = convertSync([html], removals)
    expect(result.toString()).not.toContain('Menu')
    expect(result.toString()).toContain('Article')
  })
})
