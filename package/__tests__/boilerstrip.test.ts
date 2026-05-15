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
})

describe('convert', () => {
  it('converts basic HTML to markdown', () => {
    const html = readFixture('convert/basic_article.html')
    const result = convert(html)
    expect(result).toContain('Getting Started')
  })

  it('strips boilerplate when removals are provided', () => {
    const pages = [
      '<html><body><nav class="nav">Menu</nav><main>Page 1 content</main></body></html>',
      '<html><body><nav class="nav">Menu</nav><main>Page 2 content</main></body></html>',
    ]
    const removals: Removals = learn(pages)
    const html =
      '<html><body><nav class="nav">Menu</nav><main>Article content</main></body></html>'
    const result = convert(html, removals)
    expect(result).not.toContain('Menu')
    expect(result).toContain('Article content')
  })

  it('converts without removals when none provided', () => {
    const html = '<html><body><main><h1>Hello</h1><p>World</p></main></body></html>'
    const result = convert(html)
    expect(result).toContain('Hello')
    expect(result).toContain('World')
  })
})
