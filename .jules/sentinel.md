## 2024-05-18 - XSS Filter Bypass via Whitespace and Case Sensitivity
**Vulnerability:** The `should_remove_by_href` function matched blocked URL patterns (like `javascript:`) against raw href attributes without sanitization. An attacker could bypass the filter using mixed case or leading whitespace (e.g., `  JaVaScRiPt:alert(1)`).
**Learning:** Naive substring matching on raw user input is insufficient for security filters. Browsers are lenient with URL schemes and will execute them even if they contain whitespace or mixed casing.
**Prevention:** Always normalize untrusted input (e.g., trimming whitespace and converting to lowercase) before matching against security patterns or blocklists.
