import { test, expect } from 'vitest'
import { convert } from '../index.js'

test('convert accepts missing options without throwing', async () => {
    const html = Buffer.from('<html><body><nav>Menu</nav><p>Content</p></body></html>')
    const options = {
        cssSelectorsToRemove: undefined
    }
    const result = await convert(html, options)
    expect(result.content).toContain('Menu')
    expect(result.content).toContain('Content')
})
