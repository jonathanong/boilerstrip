import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect } from 'vitest'
import { learn, convert, type Removals } from '../index'

const FIXTURES = join(__dirname, '../../fixtures')

function readFixture(path: string): string {
  return readFileSync(join(FIXTURES, path), 'utf-8')
}

describe('learn', () => {
  it('detects shared selectors from site_a', () => {
    const pages = ['page1.html', 'page2.html', 'page3.html'].map((f) =>
      readFixture(`learn/site_a/${f}`),
    )
    const removals = learn(pages)
    expect(removals.cssSelectorsToRemove).toContain('.site-nav')
    expect(removals.cssSelectorsToRemove).toContain('#site-footer')
  })

  it('throws when fewer than 2 pages are provided', () => {
    expect(() => learn(['<html></html>'])).toThrow()
  })

  it('returns css selectors and html snippets fields', () => {
    const pages = [
      '<html><body><nav class="site-nav">Menu</nav><main>Page 1</main></body></html>',
      '<html><body><nav class="site-nav">Menu</nav><main>Page 2</main></body></html>',
    ]
    const removals = learn(pages)
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
    expect(Array.isArray(removals.htmlToRemove)).toBe(true)
  })

  it('accepts custom boilerplate patterns via options', () => {
    const pages = [
      '<html><body><p class="ad">Buy now for great savings today!</p><main>Page 1 content</main></body></html>',
      '<html><body><p class="ad">Buy now for great savings today!</p><main>Page 2 content</main></body></html>',
    ]
    const removals = learn(pages, { boilerplatePatterns: ['buy now'] })
    expect(removals.cssSelectorsToRemove).toContain('.ad')
  })

  it('accepts max_selector_matches_per_page override', () => {
    // Just confirms the option is accepted without throwing.
    const pages = [
      '<html><body><nav class="site-nav">Menu</nav><main>Page 1</main></body></html>',
      '<html><body><nav class="site-nav">Menu</nav><main>Page 2</main></body></html>',
    ]
    const removals = learn(pages, { maxSelectorMatchesPerPage: 50 })
    expect(Array.isArray(removals.cssSelectorsToRemove)).toBe(true)
  })
})

describe('convert', () => {
  it('converts basic HTML to markdown', () => {
    const html = readFixture('convert/basic_article.html')
    const result = convert(html)
    expect(result).toContain('Getting Started')
  })

  it('strips boilerplate when removals are provided via options', () => {
    const pages = [
      '<html><body><nav class="nav">Menu</nav><main>Page 1 content</main></body></html>',
      '<html><body><nav class="nav">Menu</nav><main>Page 2 content</main></body></html>',
    ]
    const removals: Removals = learn(pages)
    const html =
      '<html><body><nav class="nav">Menu</nav><main>Article content</main></body></html>'
    const result = convert(html, { removals })
    expect(result).not.toContain('Menu')
    expect(result).toContain('Article content')
  })

  it('converts without options', () => {
    const html = '<html><body><main><h1>Hello</h1><p>World</p></main></body></html>'
    const result = convert(html)
    expect(result).toContain('Hello')
    expect(result).toContain('World')
  })

  it('applies content_selectors option', () => {
    const html =
      '<html><body><nav>Nav</nav><article><h1>Article heading</h1></article></body></html>'
    const result = convert(html, { contentSelectors: ['article'] })
    expect(result).toContain('Article heading')
    expect(result).not.toContain('Nav')
  })

  it('removes links matching href prefix via options', () => {
    const html =
      '<html><body><main><a href="javascript:void(0)">Click</a><a href="/safe">Safe</a></main></body></html>'
    const result = convert(html, { linkHrefsToRemove: ['javascript:'] })
    expect(result).not.toContain('javascript:')
    expect(result).toContain('Safe')
  })

  it('applies text density filter via options', () => {
    const longFooter = 'Footer text '.repeat(30)
    const html = `<html><body><footer>${longFooter}</footer><article>Real content here</article></body></html>`
    const removals: Removals = { cssSelectorsToRemove: ['footer'], htmlToRemove: [] }
    const result = convert(html, { removals, useTextDensityFilter: true })
    expect(result).not.toContain('Footer text')
    expect(result).toContain('Real content')
  })
})
