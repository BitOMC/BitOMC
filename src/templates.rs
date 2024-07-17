use {super::*, boilerplate::Boilerplate};

pub(crate) use {
  crate::subcommand::server::ServerConfig, address::AddressHtml, block::BlockHtml, clock::ClockSvg,
  home::HomeHtml, iframe::Iframe, input::InputHtml, output::OutputHtml,
};

pub use {
  blocks::BlocksHtml, rune::RuneHtml, runes::RunesHtml, status::StatusHtml,
  transaction::TransactionHtml,
};

pub mod address;
pub mod block;
pub mod blocks;
mod clock;
mod home;
mod iframe;
mod input;
pub mod output;
pub mod rune;
pub mod runes;
pub mod status;
pub mod transaction;

#[derive(Boilerplate)]
pub(crate) struct PageHtml<T: PageContent> {
  content: T,
  config: Arc<ServerConfig>,
}

impl<T> PageHtml<T>
where
  T: PageContent,
{
  pub(crate) fn new(content: T, config: Arc<ServerConfig>) -> Self {
    Self { content, config }
  }

  fn og_image(&self) -> String {
    if let Some(domain) = &self.config.domain {
      format!("https://{domain}/static/favicon.png")
    } else {
      "https://ordinals.com/static/favicon.png".into()
    }
  }

  fn superscript(&self) -> String {
    if self.config.chain == Chain::Mainnet {
      "alpha".into()
    } else {
      self.config.chain.to_string()
    }
  }
}

pub(crate) trait PageContent: Display + 'static {
  fn title(&self) -> String;

  fn page(self, server_config: Arc<ServerConfig>) -> PageHtml<Self>
  where
    Self: Sized,
  {
    PageHtml::new(self, server_config)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Foo;

  impl Display for Foo {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
      write!(f, "<h1>Foo</h1>")
    }
  }

  impl PageContent for Foo {
    fn title(&self) -> String {
      "Foo".to_string()
    }
  }

  #[test]
  fn page() {
    assert_regex_match!(
      Foo.page(Arc::new(ServerConfig {
        chain: Chain::Mainnet,
        domain: Some("signet.ordinals.com".into()),
        ..default()
      }),),
      r"<!doctype html>
<html lang=en>
  <head>
    <meta charset=utf-8>
    <meta name=format-detection content='telephone=no'>
    <meta name=viewport content='width=device-width,initial-scale=1.0'>
    <meta property=og:title content='Foo'>
    <meta property=og:image content='https://signet.ordinals.com/static/favicon.png'>
    <meta property=twitter:card content=summary>
    <title>Foo</title>
    <link rel=alternate href=/feed.xml type=application/rss\+xml title='Inscription Feed'>
    <link rel=icon href=/static/favicon.png>
    <link rel=icon href=/static/favicon.svg>
    <link rel=stylesheet href=/static/index.css>
    <link rel=stylesheet href=/static/modern-normalize.css>
    <script src=/static/index.js defer></script>
  </head>
  <body>
  <header>
    <nav>
      <a href=/ title=home>Ordinals<sup>alpha</sup></a>
      .*
      <a href=/clock title=clock>.*</a>
      .*
      <form action=/search method=get>
        <input type=text .*>
        <input class=icon type=image .*>
      </form>
    </nav>
  </header>
  <main>
<h1>Foo</h1>
  </main>
  </body>
</html>
"
    );
  }

  #[test]
  fn page_mainnet() {
    assert_regex_match!(
      Foo.page(Arc::new(ServerConfig {
        chain: Chain::Mainnet,
        domain: None,
        ..default()
      })),
      r".*<nav>\s*<a href=/ title=home>Ordinals<sup>alpha</sup></a>.*"
    );
  }

  #[test]
  fn page_no_sat_index() {
    assert_regex_match!(
      Foo.page(Arc::new(ServerConfig {
        chain: Chain::Mainnet,
        domain: None,
        ..default()
      })),
      r".*<nav>\s*<a href=/ title=home>Ordinals<sup>alpha</sup></a>.*<a href=/clock title=clock>.*</a>\s*<form action=/search.*",
    );
  }

  #[test]
  fn page_signet() {
    assert_regex_match!(
      Foo.page(Arc::new(ServerConfig {
        chain: Chain::Signet,
        domain: None,
        ..default()
      })),
      r".*<nav>\s*<a href=/ title=home>Ordinals<sup>signet</sup></a>.*"
    );
  }
}
