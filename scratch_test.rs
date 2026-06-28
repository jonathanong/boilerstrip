use scraper::{Html, Selector};

fn calculate_old(element: &scraper::ElementRef) -> i32 {
    let text: String = element.text().collect();
    let text_length = text.len() as i32;

    let mut link_text_length = 0i32;
    let mut link_count = 0i32;
    let selector_a = Selector::parse("a").unwrap();
    for link in element.select(&selector_a) {
        let link_text: String = link.text().collect();
        link_text_length += link_text.len() as i32;
        link_count += 1;
    }

    text_length - link_text_length - (link_count * 20)
}

fn calculate_new(element: &scraper::ElementRef) -> i32 {
    let mut text_length = 0;
    let mut link_text_length = 0;
    let mut link_count = 0;
    let mut link_depth = 0;

    for edge in element.traverse() {
        match edge {
            ego_tree::iter::Edge::Open(node) => {
                if let Some(el) = node.value().as_element() {
                    if el.name() == "a" {
                        link_depth += 1;
                        link_count += 1;
                    }
                } else if let Some(t) = node.value().as_text() {
                    let len = t.text.len();
                    text_length += len;
                    if link_depth > 0 {
                        link_text_length += len;
                    }
                }
            }
            ego_tree::iter::Edge::Close(node) => {
                if let Some(el) = node.value().as_element() {
                    if el.name() == "a" {
                        link_depth -= 1;
                    }
                }
            }
        }
    }

    (text_length as i32) - (link_text_length as i32) - (link_count * 20)
}

fn main() {
    let html = Html::parse_document("<html><body><div><a href='#'>Hello <a href='#'>nested</a></a></div></body></html>");
    let selector = Selector::parse("div").unwrap();
    let div = html.select(&selector).next().unwrap();
    println!("Old: {}", calculate_old(&div));
    println!("New: {}", calculate_new(&div));
}
