import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect } from 'vitest'
import { learn, convert, type Removals } from '../index'

const FIXTURES = join(__dirname, '../../fixtures')

function readFixture(path: string): string {
  return readFileSync(join(FIXTURES, path), 'utf-8')
}

describe('learn', () => {
  it('detects shared selectors from site_a', async () => {
    const pages = ['page1.html', 'page2.html', 'page3.html'].map((f) =>
      readFixture(`learn/site_a/${f}`),
    )
    const removals = await learn(pages)
    expect(removals.cssSelectorsToRemove).toContain('.site-nav')
    expect(removals.cssSelectorsToRemove).toContain('#site-footer')
  })

  it('throws when fewer than 2 pages are provided', async () => {
    await expect(learn(['<html></html>'])).rejects.toThrow()
  })

  it('returns css selectors and html snippets fields', async () => {
    const pages = [
      '<html><body><nav class="site-nav">Menu</nav><main>Page 1</main></body></html>',
      '<html><body><nav class="site-nav">Menu</nav><main>Page 2</main></body></html>',
    ]
    const removals = await learn(pages)
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
    expect(Array.isArray(removals.htmlToRemove)).toBe(true)
  })

  it('accepts custom boilerplate patterns via options', async () => {
    const pages = [
      '<html><body><p class="ad">Buy now for great savings today!</p><main>Page 1 content</main></body></html>',
      '<html><body><p class="ad">Buy now for great savings today!</p><main>Page 2 content</main></body></html>',
    ]
    const removals = await learn(pages, { boilerplatePatterns: ['buy now'] })
    expect(removals.cssSelectorsToRemove).toContain('.ad')
  })

  it('accepts max_selector_matches_per_page override', async () => {
    const pages = [
      '<html><body><nav class="site-nav">Menu</nav><main>Page 1</main></body></html>',
      '<html><body><nav class="site-nav">Menu</nav><main>Page 2</main></body></html>',
    ]
    const removals = await learn(pages, { maxSelectorMatchesPerPage: 50 })
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
  })
})

describe('convert', () => {
  it('converts basic HTML to markdown', async () => {
    const html = readFixture('convert/basic_article.html')
    const result = await convert(html)
    expect(result.content).toContain('Getting Started')
  })

  it('strips boilerplate when removals are provided via options', async () => {
    const pages = [
      '<html><body><nav class="nav">Menu</nav><main>Page 1 content</main></body></html>',
      '<html><body><nav class="nav">Menu</nav><main>Page 2 content</main></body></html>',
    ]
    const removals: Removals = await learn(pages)
    const html =
      '<html><body><nav class="nav">Menu</nav><main>Article content</main></body></html>'
    const result = await convert(html, { removals })
    expect(result.content).not.toContain('Menu')
    expect(result.content).toContain('Article content')
  })

  it('converts without options', async () => {
    const html = '<html><body><main><h1>Hello</h1><p>World</p></main></body></html>'
    const result = await convert(html)
    expect(result.content).toContain('Hello')
    expect(result.content).toContain('World')
  })

  it('applies content_selectors option', async () => {
    const html =
      '<html><body><nav>Nav</nav><article><h1>Article heading</h1></article></body></html>'
    const result = await convert(html, { contentSelectors: ['article'] })
    expect(result.content).toContain('Article heading')
    expect(result.content).not.toContain('Nav')
  })

  it('removes links matching href prefix via options', async () => {
    const html =
      '<html><body><main><a href="javascript:void(0)">Click</a><a href="/safe">Safe</a></main></body></html>'
    const result = await convert(html, { linkHrefsToRemove: ['javascript:'] })
    expect(result.content).not.toContain('javascript:')
    expect(result.content).toContain('Safe')
  })

  it('applies text density filter via options', async () => {
    const longFooter = 'Footer text '.repeat(30)
    const html = `<html><body><footer>${longFooter}</footer><article>Real content here</article></body></html>`
    const removals: Removals = { cssSelectorsToRemove: ['footer'], htmlToRemove: [] }
    const result = await convert(html, { removals, useTextDensityFilter: true })
    expect(result.content).not.toContain('Footer text')
    expect(result.content).toContain('Real content')
  })

  it('returns metadata fields', async () => {
    const html = `<html lang="en">
      <head>
        <title>My Page</title>
        <meta name="description" content="A test page" />
        <link rel="canonical" href="https://example.com/page" />
        <link rel="me" href="https://example.com/author" />
      </head>
      <body><main><p>Content here</p></main></body>
    </html>`
    const result = await convert(html)
    expect(result.title).toBe('My Page')
    expect(result.lang).toBe('en')
    expect(result.canonicalUrl).toBe('https://example.com/page')
    expect((result.meta as Record<string, string>).description).toBe('A test page')
    expect((result.link as Record<string, string>).me).toBe('https://example.com/author')
  })
})
