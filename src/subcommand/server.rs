use {
  self::{
    accept_json::AcceptJson,
    error::{OptionExt, ServerError, ServerResult},
  },
  super::*,
  crate::templates::{
    AddressHtml, BlockHtml, BlocksHtml, ClockSvg, HomeHtml, InputHtml, OutputHtml, PageContent,
    PageHtml, RuneHtml, RunesHtml, TransactionHtml,
  },
  axum::{
    body,
    extract::{DefaultBodyLimit, Extension, Json, Path, Query},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
  },
  axum_server::Handle,
  rust_embed::RustEmbed,
  rustls_acme::{
    acme::{LETS_ENCRYPT_PRODUCTION_DIRECTORY, LETS_ENCRYPT_STAGING_DIRECTORY},
    axum::AxumAcceptor,
    caches::DirCache,
    AcmeConfig,
  },
  std::{str, sync::Arc},
  tokio_stream::StreamExt,
  tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    set_header::SetResponseHeaderLayer,
    validate_request::ValidateRequestHeaderLayer,
  },
};

pub(crate) use server_config::ServerConfig;

mod accept_json;
mod error;
pub mod query;
mod server_config;

enum SpawnConfig {
  Https(AxumAcceptor),
  Http,
  Redirect(String),
}

#[derive(Deserialize)]
struct Search {
  query: String,
}

#[derive(RustEmbed)]
#[folder = "static"]
struct StaticAssets;

struct StaticHtml {
  title: &'static str,
  html: &'static str,
}

impl PageContent for StaticHtml {
  fn title(&self) -> String {
    self.title.into()
  }
}

impl Display for StaticHtml {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    f.write_str(self.html)
  }
}

#[derive(Debug, Parser, Clone)]
pub struct Server {
  #[arg(
    long,
    help = "Listen on <ADDRESS> for incoming requests. [default: 0.0.0.0]"
  )]
  pub(crate) address: Option<String>,
  #[arg(
    long,
    help = "Request ACME TLS certificate for <ACME_DOMAIN>. This ord instance must be reachable at <ACME_DOMAIN>:443 to respond to Let's Encrypt ACME challenges."
  )]
  pub(crate) acme_domain: Vec<String>,
  #[arg(long, help = "Disable JSON API.")]
  pub(crate) disable_json_api: bool,
  #[arg(
    long,
    help = "Listen on <HTTP_PORT> for incoming HTTP requests. [default: 80]"
  )]
  pub(crate) http_port: Option<u16>,
  #[arg(
    long,
    group = "port",
    help = "Listen on <HTTPS_PORT> for incoming HTTPS requests. [default: 443]"
  )]
  pub(crate) https_port: Option<u16>,
  #[arg(long, help = "Store ACME TLS certificates in <ACME_CACHE>.")]
  pub(crate) acme_cache: Option<PathBuf>,
  #[arg(long, help = "Provide ACME contact <ACME_CONTACT>.")]
  pub(crate) acme_contact: Vec<String>,
  #[arg(long, help = "Serve HTTP traffic on <HTTP_PORT>.")]
  pub(crate) http: bool,
  #[arg(long, help = "Serve HTTPS traffic on <HTTPS_PORT>.")]
  pub(crate) https: bool,
  #[arg(long, help = "Redirect HTTP traffic to HTTPS.")]
  pub(crate) redirect_http_to_https: bool,
  #[arg(long, alias = "nosync", help = "Do not update the index.")]
  pub(crate) no_sync: bool,
  #[arg(
    long,
    default_value = "5s",
    help = "Poll Bitcoin Core every <POLLING_INTERVAL>."
  )]
  pub(crate) polling_interval: humantime::Duration,
}

impl Server {
  pub fn run(self, settings: Settings, index: Arc<Index>, handle: Handle) -> SubcommandResult {
    Runtime::new()?.block_on(async {
      let index_clone = index.clone();
      let integration_test = settings.integration_test();

      let index_thread = thread::spawn(move || loop {
        if SHUTTING_DOWN.load(atomic::Ordering::Relaxed) {
          break;
        }

        if !self.no_sync {
          if let Err(error) = index_clone.update() {
            log::warn!("Updating index: {error}");
          }
        }

        thread::sleep(if integration_test {
          Duration::from_millis(100)
        } else {
          self.polling_interval.into()
        });
      });

      INDEXER.lock().unwrap().replace(index_thread);

      let settings = Arc::new(settings);
      let acme_domains = self.acme_domains()?;

      let server_config = Arc::new(ServerConfig {
        chain: settings.chain(),
        domain: acme_domains.first().cloned(),
        index_sats: index.has_sat_index(),
        json_api_enabled: !self.disable_json_api,
      });

      let router = Router::new()
        .route("/", get(Self::home))
        .route("/address/:address", get(Self::address))
        .route("/block/:query", get(Self::block))
        .route("/blockcount", get(Self::block_count))
        .route("/blockhash", get(Self::block_hash))
        .route("/blockhash/:height", get(Self::block_hash_from_height))
        .route("/blockheight", get(Self::block_height))
        .route("/blocks", get(Self::blocks))
        .route("/blocktime", get(Self::block_time))
        .route("/clock", get(Self::clock))
        .route("/faq", get(Self::faq))
        .route("/favicon.ico", get(Self::favicon))
        .route("/feed.xml", get(Self::feed))
        .route("/input/:block/:transaction/:input", get(Self::input))
        .route("/install.sh", get(Self::install_script))
        .route("/output/:output", get(Self::output))
        .route("/outputs", post(Self::outputs))
        .route("/r/blockhash", get(Self::block_hash_json))
        .route(
          "/r/blockhash/:height",
          get(Self::block_hash_from_height_json),
        )
        .route("/r/blockheight", get(Self::block_height))
        .route("/r/blocktime", get(Self::block_time))
        .route("/r/blockinfo/:query", get(Self::block_info))
        .route("/ratehistory", get(Self::rate_history))
        .route("/rune/:rune", get(Self::rune))
        .route("/runes", get(Self::runes))
        .route("/runes/:page", get(Self::runes_paginated))
        .route("/runes/balances", get(Self::runes_balances))
        .route("/search", get(Self::search_by_query))
        .route("/search/*query", get(Self::search_by_path))
        .route("/simulate", post(Self::simulate))
        .route("/static/*path", get(Self::static_asset))
        .route("/status", get(Self::status))
        .route("/tx/:txid", get(Self::transaction))
        .route("/decode/:txid", get(Self::decode))
        .route("/update", get(Self::update))
        .route("/util", get(Self::util))
        .fallback(Self::fallback)
        .layer(Extension(index))
        .layer(Extension(server_config.clone()))
        .layer(Extension(settings.clone()))
        .layer(SetResponseHeaderLayer::if_not_present(
          header::CONTENT_SECURITY_POLICY,
          HeaderValue::from_static("default-src 'self'"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
          header::STRICT_TRANSPORT_SECURITY,
          HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        ))
        .layer(
          CorsLayer::new()
            .allow_methods([http::Method::GET])
            .allow_origin(Any),
        )
        .layer(CompressionLayer::new())
        .with_state(server_config.clone());

      let router = if server_config.json_api_enabled {
        router.layer(DefaultBodyLimit::disable())
      } else {
        router
      };

      let router = if let Some((username, password)) = settings.credentials() {
        router.layer(ValidateRequestHeaderLayer::basic(username, password))
      } else {
        router
      };

      match (self.http_port(), self.https_port()) {
        (Some(http_port), None) => {
          self
            .spawn(&settings, router, handle, http_port, SpawnConfig::Http)?
            .await??
        }
        (None, Some(https_port)) => {
          self
            .spawn(
              &settings,
              router,
              handle,
              https_port,
              SpawnConfig::Https(self.acceptor(&settings)?),
            )?
            .await??
        }
        (Some(http_port), Some(https_port)) => {
          let http_spawn_config = if self.redirect_http_to_https {
            SpawnConfig::Redirect(if https_port == 443 {
              format!("https://{}", acme_domains[0])
            } else {
              format!("https://{}:{https_port}", acme_domains[0])
            })
          } else {
            SpawnConfig::Http
          };

          let (http_result, https_result) = tokio::join!(
            self.spawn(
              &settings,
              router.clone(),
              handle.clone(),
              http_port,
              http_spawn_config
            )?,
            self.spawn(
              &settings,
              router,
              handle,
              https_port,
              SpawnConfig::Https(self.acceptor(&settings)?),
            )?
          );
          http_result.and(https_result)??;
        }
        (None, None) => unreachable!(),
      }

      Ok(None)
    })
  }

  fn spawn(
    &self,
    settings: &Settings,
    router: Router,
    handle: Handle,
    port: u16,
    config: SpawnConfig,
  ) -> Result<task::JoinHandle<io::Result<()>>> {
    let address = match &self.address {
      Some(address) => address.as_str(),
      None => {
        if cfg!(test) || settings.integration_test() {
          "127.0.0.1"
        } else {
          "0.0.0.0"
        }
      }
    };

    let addr = (address, port)
      .to_socket_addrs()?
      .next()
      .ok_or_else(|| anyhow!("failed to get socket addrs"))?;

    if !settings.integration_test() && !cfg!(test) {
      eprintln!(
        "Listening on {}://{addr}",
        match config {
          SpawnConfig::Https(_) => "https",
          _ => "http",
        }
      );
    }

    Ok(tokio::spawn(async move {
      match config {
        SpawnConfig::Https(acceptor) => {
          axum_server::Server::bind(addr)
            .handle(handle)
            .acceptor(acceptor)
            .serve(router.into_make_service())
            .await
        }
        SpawnConfig::Redirect(destination) => {
          axum_server::Server::bind(addr)
            .handle(handle)
            .serve(
              Router::new()
                .fallback(Self::redirect_http_to_https)
                .layer(Extension(destination))
                .into_make_service(),
            )
            .await
        }
        SpawnConfig::Http => {
          axum_server::Server::bind(addr)
            .handle(handle)
            .serve(router.into_make_service())
            .await
        }
      }
    }))
  }

  fn acme_cache(acme_cache: Option<&PathBuf>, settings: &Settings) -> PathBuf {
    match acme_cache {
      Some(acme_cache) => acme_cache.clone(),
      None => settings.data_dir().join("acme-cache"),
    }
  }

  fn acme_domains(&self) -> Result<Vec<String>> {
    if !self.acme_domain.is_empty() {
      Ok(self.acme_domain.clone())
    } else {
      Ok(vec![
        System::host_name().ok_or(anyhow!("no hostname found"))?
      ])
    }
  }

  fn http_port(&self) -> Option<u16> {
    if self.http || self.http_port.is_some() || (self.https_port.is_none() && !self.https) {
      Some(self.http_port.unwrap_or(80))
    } else {
      None
    }
  }

  fn https_port(&self) -> Option<u16> {
    if self.https || self.https_port.is_some() {
      Some(self.https_port.unwrap_or(443))
    } else {
      None
    }
  }

  fn acceptor(&self, settings: &Settings) -> Result<AxumAcceptor> {
    let config = AcmeConfig::new(self.acme_domains()?)
      .contact(&self.acme_contact)
      .cache_option(Some(DirCache::new(Self::acme_cache(
        self.acme_cache.as_ref(),
        settings,
      ))))
      .directory(if cfg!(test) {
        LETS_ENCRYPT_STAGING_DIRECTORY
      } else {
        LETS_ENCRYPT_PRODUCTION_DIRECTORY
      });

    let mut state = config.state();

    let mut server_config = rustls::ServerConfig::builder()
      .with_no_client_auth()
      .with_cert_resolver(state.resolver());

    server_config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];

    let acceptor = state.axum_acceptor(Arc::new(server_config));

    tokio::spawn(async move {
      while let Some(result) = state.next().await {
        match result {
          Ok(ok) => log::info!("ACME event: {:?}", ok),
          Err(err) => log::error!("ACME error: {:?}", err),
        }
      }
    });

    Ok(acceptor)
  }

  fn index_height(index: &Index) -> ServerResult<Height> {
    index.block_height()?.ok_or_not_found(|| "genesis block")
  }

  async fn clock(Extension(index): Extension<Arc<Index>>) -> ServerResult {
    task::block_in_place(|| {
      Ok(
        (
          [(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'unsafe-inline'"),
          )],
          ClockSvg::new(Self::index_height(&index)?),
        )
          .into_response(),
      )
    })
  }

  async fn fallback(Extension(index): Extension<Arc<Index>>, uri: Uri) -> ServerResult<Response> {
    task::block_in_place(|| {
      let path = urlencoding::decode(uri.path().trim_matches('/'))
        .map_err(|err| ServerError::BadRequest(err.to_string()))?;

      let prefix = if re::INSCRIPTION_ID.is_match(&path) || re::INSCRIPTION_NUMBER.is_match(&path) {
        "inscription"
      } else if re::RUNE_ID.is_match(&path) || re::SPACED_RUNE.is_match(&path) {
        "rune"
      } else if re::OUTPOINT.is_match(&path) {
        "output"
      } else if re::HASH.is_match(&path) {
        if index.block_header(path.parse().unwrap())?.is_some() {
          "block"
        } else {
          "tx"
        }
      } else {
        return Ok(StatusCode::NOT_FOUND.into_response());
      };

      Ok(Redirect::to(&format!("/{prefix}/{path}")).into_response())
    })
  }

  async fn output(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(outpoint): Path<OutPoint>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let (output_info, txout) = index
        .get_output_info(outpoint)?
        .ok_or_not_found(|| format!("output {outpoint}"))?;

      Ok(if accept_json {
        Json(output_info).into_response()
      } else {
        OutputHtml {
          chain: server_config.chain,
          inscriptions: output_info.inscriptions,
          outpoint,
          output: txout,
          runes: output_info.runes,
          sat_ranges: output_info.sat_ranges,
          spent: output_info.spent,
        }
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn outputs(
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
    Json(outputs): Json<Vec<OutPoint>>,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        let mut response = Vec::new();
        for outpoint in outputs {
          let (output_info, _) = index
            .get_output_info(outpoint)?
            .ok_or_not_found(|| format!("output {outpoint}"))?;

          response.push(output_info);
        }
        Json(response).into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn simulate(
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
    Json(transactions): Json<Vec<Transaction>>,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        let simulation = index.simulate(transactions)?;
        Json(simulation).into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn rune(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(DeserializeFromStr(rune_query)): Path<DeserializeFromStr<query::Rune>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      if !index.has_rune_index() {
        return Err(ServerError::NotFound(
          "this server has no rune index".to_string(),
        ));
      }

      let rune = match rune_query {
        query::Rune::Spaced(spaced_rune) => spaced_rune.rune,
        query::Rune::Id(rune_id) => index
          .get_rune_by_id(rune_id)?
          .ok_or_not_found(|| format!("rune {rune_id}"))?,
        query::Rune::Number(number) => index
          .get_rune_by_number(usize::try_from(number).unwrap())?
          .ok_or_not_found(|| format!("rune number {number}"))?,
      };

      let (id, entry) = index
        .rune(rune)?
        .ok_or_not_found(|| format!("rune {rune}"))?;

      let mintable = true;

      Ok(if accept_json {
        Json(api::Rune {
          entry,
          id,
          mintable,
          parent: None,
        })
        .into_response()
      } else {
        RuneHtml {
          entry,
          id,
          mintable,
          parent: None,
        }
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn runes(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    accept_json: AcceptJson,
  ) -> ServerResult<Response> {
    Self::runes_paginated(
      Extension(server_config),
      Extension(index),
      Path(0),
      accept_json,
    )
    .await
  }

  async fn runes_paginated(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(page_index): Path<usize>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let (entries, more) = index.runes_paginated(50, page_index)?;

      let prev = page_index.checked_sub(1);

      let next = more.then_some(page_index + 1);

      Ok(if accept_json {
        Json(RunesHtml {
          entries,
          more,
          prev,
          next,
        })
        .into_response()
      } else {
        RunesHtml {
          entries,
          more,
          prev,
          next,
        }
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn runes_balances(
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        Json(
          index
            .get_rune_balance_map()?
            .into_iter()
            .map(|(rune, balances)| {
              (
                rune,
                balances
                  .into_iter()
                  .map(|(outpoint, pile)| (outpoint, pile.amount))
                  .collect(),
              )
            })
            .collect::<BTreeMap<SpacedRune, BTreeMap<OutPoint, u128>>>(),
        )
        .into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn home(
    Extension(server_config): Extension<Arc<ServerConfig>>,
  ) -> ServerResult<PageHtml<HomeHtml>> {
    task::block_in_place(|| {
      Ok(
        HomeHtml {
          inscriptions: vec![],
        }
        .page(server_config),
      )
    })
  }

  async fn blocks(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let blocks = index.blocks(100)?;
      let mut featured_blocks = BTreeMap::new();
      for (_height, hash) in blocks.iter().take(5) {
        featured_blocks.insert(*hash, vec![]);
      }

      Ok(if accept_json {
        Json(api::Blocks::new(blocks, featured_blocks)).into_response()
      } else {
        BlocksHtml::new(blocks, featured_blocks)
          .page(server_config)
          .into_response()
      })
    })
  }

  async fn install_script() -> Redirect {
    Redirect::to("https://raw.githubusercontent.com/ordinals/ord/master/install.sh")
  }

  async fn address(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(address): Path<Address<NetworkUnchecked>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      if !index.has_address_index() {
        return Err(ServerError::NotFound(
          "this server has no address index".to_string(),
        ));
      }

      let address = address
        .require_network(server_config.chain.network())
        .map_err(|err| ServerError::BadRequest(err.to_string()))?;

      let mut outputs = index.get_address_info(&address)?;

      outputs.sort();

      let sat_balance = index.get_sat_balances_for_outputs(&outputs)?;

      Ok(if accept_json {
        Json(outputs).into_response()
      } else {
        AddressHtml {
          address,
          outputs,
          sat_balance,
        }
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn block(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(DeserializeFromStr(query)): Path<DeserializeFromStr<query::Block>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let (block, height) = match query {
        query::Block::Height(height) => {
          let block = index
            .get_block_by_height(height)?
            .ok_or_not_found(|| format!("block {height}"))?;

          (block, height)
        }
        query::Block::Hash(hash) => {
          let info = index
            .block_header_info(hash)?
            .ok_or_not_found(|| format!("block {hash}"))?;

          let block = index
            .get_block_by_hash(hash)?
            .ok_or_not_found(|| format!("block {hash}"))?;

          (block, u32::try_from(info.height).unwrap())
        }
      };

      let runes = index.get_runes_in_block(u64::from(height))?;
      Ok(if accept_json {
        Json(api::Block::new(
          block,
          Height(height),
          Self::index_height(&index)?,
          vec![],
          runes,
        ))
        .into_response()
      } else {
        BlockHtml::new(
          block,
          Height(height),
          Self::index_height(&index)?,
          0,
          vec![],
          runes,
        )
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn transaction(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(txid): Path<Txid>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let transaction = index
        .get_transaction(txid)?
        .ok_or_not_found(|| format!("transaction {txid}"))?;

      let inscription_count = 0;

      Ok(if accept_json {
        Json(api::Transaction {
          chain: server_config.chain,
          etching: None,
          inscription_count,
          transaction,
          txid,
        })
        .into_response()
      } else {
        TransactionHtml {
          chain: server_config.chain,
          etching: None,
          inscription_count,
          transaction,
          txid,
        }
        .page(server_config)
        .into_response()
      })
    })
  }

  async fn decode(
    Extension(index): Extension<Arc<Index>>,
    Path(txid): Path<Txid>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      let transaction = index
        .get_transaction(txid)?
        .ok_or_not_found(|| format!("transaction {txid}"))?;

      let runestone = Runestone::decipher(&transaction);

      Ok(if accept_json {
        Json(api::Decode { runestone }).into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn update(
    Extension(settings): Extension<Arc<Settings>>,
    Extension(index): Extension<Arc<Index>>,
  ) -> ServerResult {
    task::block_in_place(|| {
      if settings.integration_test() {
        index.update()?;
        Ok(index.block_count()?.to_string().into_response())
      } else {
        Ok(StatusCode::NOT_FOUND.into_response())
      }
    })
  }

  async fn util(
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        Json(index.get_util_state()?).into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn rate_history(
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        Json(index.get_rate_history()?).into_response()
      } else {
        StatusCode::NOT_FOUND.into_response()
      })
    })
  }

  async fn status(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    AcceptJson(accept_json): AcceptJson,
  ) -> ServerResult {
    task::block_in_place(|| {
      Ok(if accept_json {
        Json(index.status()?).into_response()
      } else {
        index.status()?.page(server_config).into_response()
      })
    })
  }

  async fn search_by_query(
    Extension(index): Extension<Arc<Index>>,
    Query(search): Query<Search>,
  ) -> ServerResult<Redirect> {
    Self::search(index, search.query).await
  }

  async fn search_by_path(
    Extension(index): Extension<Arc<Index>>,
    Path(search): Path<Search>,
  ) -> ServerResult<Redirect> {
    Self::search(index, search.query).await
  }

  async fn search(index: Arc<Index>, query: String) -> ServerResult<Redirect> {
    Self::search_inner(index, query).await
  }

  async fn search_inner(index: Arc<Index>, query: String) -> ServerResult<Redirect> {
    task::block_in_place(|| {
      let query = query.trim();

      if re::HASH.is_match(query) {
        if index.block_header(query.parse().unwrap())?.is_some() {
          Ok(Redirect::to(&format!("/block/{query}")))
        } else {
          Ok(Redirect::to(&format!("/tx/{query}")))
        }
      } else if re::OUTPOINT.is_match(query) {
        Ok(Redirect::to(&format!("/output/{query}")))
      } else if re::INSCRIPTION_ID.is_match(query) || re::INSCRIPTION_NUMBER.is_match(query) {
        Ok(Redirect::to(&format!("/inscription/{query}")))
      } else if re::SPACED_RUNE.is_match(query) {
        Ok(Redirect::to(&format!("/rune/{query}")))
      } else if re::RUNE_ID.is_match(query) {
        let id = query
          .parse::<RuneId>()
          .map_err(|err| ServerError::BadRequest(err.to_string()))?;

        let rune = index.get_rune_by_id(id)?.ok_or_not_found(|| "rune ID")?;

        Ok(Redirect::to(&format!("/rune/{rune}")))
      } else if re::ADDRESS.is_match(query) {
        Ok(Redirect::to(&format!("/address/{query}")))
      } else {
        Ok(Redirect::to(&format!("/sat/{query}")))
      }
    })
  }

  async fn favicon() -> ServerResult {
    Ok(
      Self::static_asset(Path("/favicon.png".to_string()))
        .await
        .into_response(),
    )
  }

  async fn feed(Extension(server_config): Extension<Arc<ServerConfig>>) -> ServerResult {
    task::block_in_place(|| {
      let mut builder = rss::ChannelBuilder::default();

      let chain = server_config.chain;
      match chain {
        Chain::Mainnet => builder.title("Inscriptions".to_string()),
        _ => builder.title(format!("Inscriptions – {chain:?}")),
      };

      builder.generator(Some("ord".to_string()));

      Ok(
        (
          [
            (header::CONTENT_TYPE, "application/rss+xml"),
            (
              header::CONTENT_SECURITY_POLICY,
              "default-src 'unsafe-inline'",
            ),
          ],
          builder.build().to_string(),
        )
          .into_response(),
      )
    })
  }

  async fn static_asset(Path(path): Path<String>) -> ServerResult {
    let content = StaticAssets::get(if let Some(stripped) = path.strip_prefix('/') {
      stripped
    } else {
      &path
    })
    .ok_or_not_found(|| format!("asset {path}"))?;
    let body = body::boxed(body::Full::from(content.data));
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Ok(
      Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(body)
        .unwrap(),
    )
  }

  async fn block_count(Extension(index): Extension<Arc<Index>>) -> ServerResult<String> {
    task::block_in_place(|| Ok(index.block_count()?.to_string()))
  }

  async fn block_height(Extension(index): Extension<Arc<Index>>) -> ServerResult<String> {
    task::block_in_place(|| {
      Ok(
        index
          .block_height()?
          .ok_or_not_found(|| "blockheight")?
          .to_string(),
      )
    })
  }

  async fn block_hash(Extension(index): Extension<Arc<Index>>) -> ServerResult<String> {
    task::block_in_place(|| {
      Ok(
        index
          .block_hash(None)?
          .ok_or_not_found(|| "blockhash")?
          .to_string(),
      )
    })
  }

  async fn block_hash_json(Extension(index): Extension<Arc<Index>>) -> ServerResult<Json<String>> {
    task::block_in_place(|| {
      Ok(Json(
        index
          .block_hash(None)?
          .ok_or_not_found(|| "blockhash")?
          .to_string(),
      ))
    })
  }

  async fn block_hash_from_height(
    Extension(index): Extension<Arc<Index>>,
    Path(height): Path<u32>,
  ) -> ServerResult<String> {
    task::block_in_place(|| {
      Ok(
        index
          .block_hash(Some(height))?
          .ok_or_not_found(|| "blockhash")?
          .to_string(),
      )
    })
  }

  async fn block_hash_from_height_json(
    Extension(index): Extension<Arc<Index>>,
    Path(height): Path<u32>,
  ) -> ServerResult<Json<String>> {
    task::block_in_place(|| {
      Ok(Json(
        index
          .block_hash(Some(height))?
          .ok_or_not_found(|| "blockhash")?
          .to_string(),
      ))
    })
  }

  async fn block_info(
    Extension(index): Extension<Arc<Index>>,
    Path(DeserializeFromStr(query)): Path<DeserializeFromStr<query::Block>>,
  ) -> ServerResult<Json<api::BlockInfo>> {
    task::block_in_place(|| {
      let hash = match query {
        query::Block::Hash(hash) => hash,
        query::Block::Height(height) => index
          .block_hash(Some(height))?
          .ok_or_not_found(|| format!("block {height}"))?,
      };

      let header = index
        .block_header(hash)?
        .ok_or_not_found(|| format!("block {hash}"))?;

      let info = index
        .block_header_info(hash)?
        .ok_or_not_found(|| format!("block {hash}"))?;

      let stats = index
        .block_stats(info.height.try_into().unwrap())?
        .ok_or_not_found(|| format!("block {hash}"))?;

      Ok(Json(api::BlockInfo {
        average_fee: stats.avg_fee.to_sat(),
        average_fee_rate: stats.avg_fee_rate.to_sat(),
        bits: header.bits.to_consensus(),
        chainwork: info.chainwork.try_into().unwrap(),
        confirmations: info.confirmations,
        difficulty: info.difficulty,
        hash,
        feerate_percentiles: [
          stats.fee_rate_percentiles.fr_10th.to_sat(),
          stats.fee_rate_percentiles.fr_25th.to_sat(),
          stats.fee_rate_percentiles.fr_50th.to_sat(),
          stats.fee_rate_percentiles.fr_75th.to_sat(),
          stats.fee_rate_percentiles.fr_90th.to_sat(),
        ],
        height: info.height.try_into().unwrap(),
        max_fee: stats.max_fee.to_sat(),
        max_fee_rate: stats.max_fee_rate.to_sat(),
        max_tx_size: stats.max_tx_size,
        median_fee: stats.median_fee.to_sat(),
        median_time: info
          .median_time
          .map(|median_time| median_time.try_into().unwrap()),
        merkle_root: info.merkle_root,
        min_fee: stats.min_fee.to_sat(),
        min_fee_rate: stats.min_fee_rate.to_sat(),
        next_block: info.next_block_hash,
        nonce: info.nonce,
        previous_block: info.previous_block_hash,
        subsidy: stats.subsidy.to_sat(),
        target: target_as_block_hash(header.target()),
        timestamp: info.time.try_into().unwrap(),
        total_fee: stats.total_fee.to_sat(),
        total_size: stats.total_size,
        total_weight: stats.total_weight,
        transaction_count: info.n_tx.try_into().unwrap(),
        #[allow(clippy::cast_sign_loss)]
        version: info.version.to_consensus() as u32,
      }))
    })
  }

  async fn block_time(Extension(index): Extension<Arc<Index>>) -> ServerResult<String> {
    task::block_in_place(|| {
      Ok(
        index
          .block_time(index.block_height()?.ok_or_not_found(|| "blocktime")?)?
          .unix_timestamp()
          .to_string(),
      )
    })
  }

  async fn input(
    Extension(server_config): Extension<Arc<ServerConfig>>,
    Extension(index): Extension<Arc<Index>>,
    Path(path): Path<(u32, usize, usize)>,
  ) -> ServerResult<PageHtml<InputHtml>> {
    task::block_in_place(|| {
      let not_found = || format!("input /{}/{}/{}", path.0, path.1, path.2);

      let block = index
        .get_block_by_height(path.0)?
        .ok_or_not_found(not_found)?;

      let transaction = block
        .txdata
        .into_iter()
        .nth(path.1)
        .ok_or_not_found(not_found)?;

      let input = transaction
        .input
        .into_iter()
        .nth(path.2)
        .ok_or_not_found(not_found)?;

      Ok(InputHtml { path, input }.page(server_config))
    })
  }

  async fn faq() -> Redirect {
    Redirect::to("https://docs.ordinals.com/faq/")
  }

  async fn redirect_http_to_https(
    Extension(mut destination): Extension<String>,
    uri: Uri,
  ) -> Redirect {
    if let Some(path_and_query) = uri.path_and_query() {
      destination.push_str(path_and_query.as_str());
    }

    Redirect::to(&destination)
  }
}

#[cfg(test)]
mod tests {
  use {super::*, reqwest::Url, std::net::TcpListener, tempfile::TempDir};

  #[derive(Default)]
  struct Builder {
    core: Option<mockcore::Handle>,
    config: String,
    ord_args: BTreeMap<String, Option<String>>,
    server_args: BTreeMap<String, Option<String>>,
  }

  impl Builder {
    fn ord_option(mut self, option: &str, value: &str) -> Self {
      self.ord_args.insert(option.into(), Some(value.into()));
      self
    }

    fn ord_flag(mut self, flag: &str) -> Self {
      self.ord_args.insert(flag.into(), None);
      self
    }

    fn server_flag(mut self, flag: &str) -> Self {
      self.server_args.insert(flag.into(), None);
      self
    }

    fn chain(self, chain: Chain) -> Self {
      self.ord_option("--chain", &chain.to_string())
    }

    fn build(self) -> TestServer {
      let core = self.core.unwrap_or_else(|| {
        mockcore::builder()
          .network(
            self
              .ord_args
              .get("--chain")
              .map(|chain| chain.as_ref().unwrap().parse::<Chain>().unwrap())
              .unwrap_or_default()
              .network(),
          )
          .build()
      });

      let tempdir = TempDir::new().unwrap();

      let cookiefile = tempdir.path().join("cookie");

      fs::write(&cookiefile, "username:password").unwrap();

      let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

      let mut args = vec!["ord".to_string()];

      args.push("--bitcoin-rpc-url".into());
      args.push(core.url());

      args.push("--cookie-file".into());
      args.push(cookiefile.to_str().unwrap().into());

      args.push("--datadir".into());
      args.push(tempdir.path().to_str().unwrap().into());

      if !self.ord_args.contains_key("--chain") {
        args.push("--chain".into());
        args.push(core.network());
      }

      for (arg, value) in self.ord_args {
        args.push(arg);

        if let Some(value) = value {
          args.push(value);
        }
      }

      args.push("server".into());

      args.push("--address".into());
      args.push("127.0.0.1".into());

      args.push("--http-port".into());
      args.push(port.to_string());

      args.push("--polling-interval".into());
      args.push("100ms".into());

      for (arg, value) in self.server_args {
        args.push(arg);

        if let Some(value) = value {
          args.push(value);
        }
      }

      let arguments = Arguments::try_parse_from(args).unwrap();

      let Subcommand::Server(server) = arguments.subcommand else {
        panic!("unexpected subcommand: {:?}", arguments.subcommand);
      };

      let settings = Settings::from_options(arguments.options)
        .or(serde_yaml::from_str::<Settings>(&self.config).unwrap())
        .or_defaults()
        .unwrap();

      let index = Arc::new(Index::open(&settings).unwrap());
      let ord_server_handle = Handle::new();

      {
        let index = index.clone();
        let ord_server_handle = ord_server_handle.clone();
        thread::spawn(|| server.run(settings, index, ord_server_handle).unwrap());
      }

      while index.statistic(crate::index::Statistic::Commits) == 0 {
        thread::sleep(Duration::from_millis(50));
      }

      let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

      for i in 0.. {
        match client.get(format!("http://127.0.0.1:{port}/status")).send() {
          Ok(_) => break,
          Err(err) => {
            if i == 400 {
              panic!("ord server failed to start: {err}");
            }
          }
        }

        thread::sleep(Duration::from_millis(50));
      }

      TestServer {
        core,
        index,
        ord_server_handle,
        tempdir,
        url: Url::parse(&format!("http://127.0.0.1:{port}")).unwrap(),
      }
    }

    fn https(self) -> Self {
      self.server_flag("--https")
    }

    #[allow(dead_code)]
    fn index_runes(self) -> Self {
      self.ord_flag("--index-runes")
    }

    fn redirect_http_to_https(self) -> Self {
      self.server_flag("--redirect-http-to-https")
    }
  }

  struct TestServer {
    core: mockcore::Handle,
    index: Arc<Index>,
    ord_server_handle: Handle,
    #[allow(unused)]
    tempdir: TempDir,
    url: Url,
  }

  impl TestServer {
    fn builder() -> Builder {
      Default::default()
    }

    fn new() -> Self {
      Builder::default().build()
    }

    #[track_caller]
    fn get(&self, path: impl AsRef<str>) -> reqwest::blocking::Response {
      if let Err(error) = self.index.update() {
        log::error!("{error}");
      }
      reqwest::blocking::get(self.join_url(path.as_ref())).unwrap()
    }

    fn join_url(&self, url: &str) -> Url {
      self.url.join(url).unwrap()
    }

    #[track_caller]
    fn assert_response(&self, path: impl AsRef<str>, status: StatusCode, expected_response: &str) {
      let response = self.get(path);
      assert_eq!(response.status(), status, "{}", response.text().unwrap());
      pretty_assert_eq!(response.text().unwrap(), expected_response);
    }

    #[track_caller]
    fn assert_response_regex(
      &self,
      path: impl AsRef<str>,
      status: StatusCode,
      regex: impl AsRef<str>,
    ) {
      let response = self.get(path);
      assert_eq!(
        response.status(),
        status,
        "response: {}",
        response.text().unwrap()
      );
      assert_regex_match!(response.text().unwrap(), regex.as_ref());
    }

    #[track_caller]
    fn assert_redirect(&self, path: &str, location: &str) {
      let response = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(self.join_url(path))
        .send()
        .unwrap();

      assert_eq!(response.status(), StatusCode::SEE_OTHER);
      assert_eq!(response.headers().get(header::LOCATION).unwrap(), location);
    }

    #[track_caller]
    fn mine_blocks(&self, n: u64) -> Vec<Block> {
      let blocks = self.core.mine_blocks(n);
      self.index.update().unwrap();
      blocks
    }
  }

  impl Drop for TestServer {
    fn drop(&mut self) {
      self.ord_server_handle.shutdown();
    }
  }

  fn parse_server_args(args: &str) -> (Settings, Server) {
    match Arguments::try_parse_from(args.split_whitespace()) {
      Ok(arguments) => match arguments.subcommand {
        Subcommand::Server(server) => (
          Settings::from_options(arguments.options)
            .or_defaults()
            .unwrap(),
          server,
        ),
        subcommand => panic!("unexpected subcommand: {subcommand:?}"),
      },
      Err(err) => panic!("error parsing arguments: {err}"),
    }
  }

  #[test]
  fn http_and_https_port_dont_conflict() {
    parse_server_args(
      "ord server --http-port 0 --https-port 0 --acme-cache foo --acme-contact bar --acme-domain baz",
    );
  }

  #[test]
  fn http_port_defaults_to_80() {
    assert_eq!(parse_server_args("ord server").1.http_port(), Some(80));
  }

  #[test]
  fn https_port_defaults_to_none() {
    assert_eq!(parse_server_args("ord server").1.https_port(), None);
  }

  #[test]
  fn https_sets_https_port_to_443() {
    assert_eq!(
      parse_server_args("ord server --https --acme-cache foo --acme-contact bar --acme-domain baz")
        .1
        .https_port(),
      Some(443)
    );
  }

  #[test]
  fn https_disables_http() {
    assert_eq!(
      parse_server_args("ord server --https --acme-cache foo --acme-contact bar --acme-domain baz")
        .1
        .http_port(),
      None
    );
  }

  #[test]
  fn https_port_disables_http() {
    assert_eq!(
      parse_server_args(
        "ord server --https-port 433 --acme-cache foo --acme-contact bar --acme-domain baz"
      )
      .1
      .http_port(),
      None
    );
  }

  #[test]
  fn https_port_sets_https_port() {
    assert_eq!(
      parse_server_args(
        "ord server --https-port 1000 --acme-cache foo --acme-contact bar --acme-domain baz"
      )
      .1
      .https_port(),
      Some(1000)
    );
  }

  #[test]
  fn http_with_https_leaves_http_enabled() {
    assert_eq!(
      parse_server_args(
        "ord server --https --http --acme-cache foo --acme-contact bar --acme-domain baz"
      )
      .1
      .http_port(),
      Some(80)
    );
  }

  #[test]
  fn http_with_https_leaves_https_enabled() {
    assert_eq!(
      parse_server_args(
        "ord server --https --http --acme-cache foo --acme-contact bar --acme-domain baz"
      )
      .1
      .https_port(),
      Some(443)
    );
  }

  #[test]
  fn acme_contact_accepts_multiple_values() {
    assert!(Arguments::try_parse_from([
      "ord",
      "server",
      "--address",
      "127.0.0.1",
      "--http-port",
      "0",
      "--acme-contact",
      "foo",
      "--acme-contact",
      "bar"
    ])
    .is_ok());
  }

  #[test]
  fn acme_domain_accepts_multiple_values() {
    assert!(Arguments::try_parse_from([
      "ord",
      "server",
      "--address",
      "127.0.0.1",
      "--http-port",
      "0",
      "--acme-domain",
      "foo",
      "--acme-domain",
      "bar"
    ])
    .is_ok());
  }

  #[test]
  fn acme_cache_defaults_to_data_dir() {
    let arguments = Arguments::try_parse_from(["ord", "--datadir", "foo", "server"]).unwrap();

    let settings = Settings::from_options(arguments.options)
      .or_defaults()
      .unwrap();

    let acme_cache = Server::acme_cache(None, &settings).display().to_string();
    assert!(
      acme_cache.contains(if cfg!(windows) {
        r"foo\acme-cache"
      } else {
        "foo/acme-cache"
      }),
      "{acme_cache}"
    )
  }

  #[test]
  fn acme_cache_flag_is_respected() {
    let arguments =
      Arguments::try_parse_from(["ord", "--datadir", "foo", "server", "--acme-cache", "bar"])
        .unwrap();

    let settings = Settings::from_options(arguments.options)
      .or_defaults()
      .unwrap();

    let acme_cache = Server::acme_cache(Some(&"bar".into()), &settings)
      .display()
      .to_string();
    assert_eq!(acme_cache, "bar")
  }

  #[test]
  fn acme_domain_defaults_to_hostname() {
    let (_, server) = parse_server_args("ord server");
    assert_eq!(
      server.acme_domains().unwrap(),
      &[System::host_name().unwrap()]
    );
  }

  #[test]
  fn acme_domain_flag_is_respected() {
    let (_, server) = parse_server_args("ord server --acme-domain example.com");
    assert_eq!(server.acme_domains().unwrap(), &["example.com"]);
  }

  #[test]
  fn install_sh_redirects_to_github() {
    TestServer::new().assert_redirect(
      "/install.sh",
      "https://raw.githubusercontent.com/ordinals/ord/master/install.sh",
    );
  }

  #[test]
  fn faq_redirects_to_docs_site() {
    TestServer::new().assert_redirect("/faq", "https://docs.ordinals.com/faq/");
  }

  #[test]
  fn search_by_query_returns_rune() {
    TestServer::new().assert_redirect("/search?query=ABCD", "/rune/ABCD");
  }

  #[test]
  fn search_by_query_returns_spaced_rune() {
    TestServer::new().assert_redirect("/search?query=AB•CD", "/rune/AB•CD");
  }

  #[test]
  fn search_by_query_returns_inscription() {
    TestServer::new().assert_redirect(
      "/search?query=0000000000000000000000000000000000000000000000000000000000000000i0",
      "/inscription/0000000000000000000000000000000000000000000000000000000000000000i0",
    );
  }

  #[test]
  fn search_by_query_returns_inscription_by_number() {
    TestServer::new().assert_redirect("/search?query=0", "/inscription/0");
  }

  #[test]
  fn search_is_whitespace_insensitive() {
    TestServer::new().assert_redirect("/search/ abc ", "/sat/abc");
  }

  #[test]
  fn search_by_path_returns_sat() {
    TestServer::new().assert_redirect("/search/abc", "/sat/abc");
  }

  #[test]
  fn search_for_blockhash_returns_block() {
    TestServer::new().assert_redirect(
      "/search/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
      "/block/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
    );
  }

  #[test]
  fn search_for_txid_returns_transaction() {
    TestServer::new().assert_redirect(
      "/search/0000000000000000000000000000000000000000000000000000000000000000",
      "/tx/0000000000000000000000000000000000000000000000000000000000000000",
    );
  }

  #[test]
  fn search_for_outpoint_returns_output() {
    TestServer::new().assert_redirect(
      "/search/0000000000000000000000000000000000000000000000000000000000000000:0",
      "/output/0000000000000000000000000000000000000000000000000000000000000000:0",
    );
  }

  #[test]
  fn search_for_inscription_id_returns_inscription() {
    TestServer::new().assert_redirect(
      "/search/0000000000000000000000000000000000000000000000000000000000000000i0",
      "/inscription/0000000000000000000000000000000000000000000000000000000000000000i0",
    );
  }

  #[test]
  fn html_runes_balances_not_found() {
    TestServer::builder()
      .chain(Chain::Regtest)
      .build()
      .assert_response("/runes/balances", StatusCode::NOT_FOUND, "");
  }

  #[test]
  fn fallback() {
    let server = TestServer::new();

    server.assert_redirect("/0", "/inscription/0");
    server.assert_redirect("/0/", "/inscription/0");
    server.assert_redirect("/0//", "/inscription/0");
    server.assert_redirect(
      "/521f8eccffa4c41a3a7728dd012ea5a4a02feed81f41159231251ecf1e5c79dai0",
      "/inscription/521f8eccffa4c41a3a7728dd012ea5a4a02feed81f41159231251ecf1e5c79dai0",
    );
    server.assert_redirect("/-1", "/inscription/-1");
    server.assert_redirect("/FOO", "/rune/FOO");
    server.assert_redirect("/FO.O", "/rune/FO.O");
    server.assert_redirect("/FO•O", "/rune/FO•O");
    server.assert_redirect("/0:0", "/rune/0:0");
    server.assert_redirect(
      "/4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b:0",
      "/output/4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b:0",
    );
    server.assert_redirect(
      "/search/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
      "/block/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
    );
    server.assert_redirect(
      "/search/0000000000000000000000000000000000000000000000000000000000000000",
      "/tx/0000000000000000000000000000000000000000000000000000000000000000",
    );

    server.assert_response_regex("/hello", StatusCode::NOT_FOUND, "");

    server.assert_response_regex(
      "/%C3%28",
      StatusCode::BAD_REQUEST,
      "invalid utf-8 sequence of 1 bytes from index 0",
    );
  }

  #[test]
  fn http_to_https_redirect_with_path() {
    TestServer::builder()
      .redirect_http_to_https()
      .https()
      .build()
      .assert_redirect(
        "/sat/0",
        &format!("https://{}/sat/0", System::host_name().unwrap()),
      );
  }

  #[test]
  fn http_to_https_redirect_with_empty() {
    TestServer::builder()
      .redirect_http_to_https()
      .https()
      .build()
      .assert_redirect("/", &format!("https://{}/", System::host_name().unwrap()));
  }

  #[test]
  fn block_count_endpoint() {
    let test_server = TestServer::new();

    let response = test_server.get("/blockcount");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().unwrap(), "1");

    test_server.mine_blocks(1);

    let response = test_server.get("/blockcount");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().unwrap(), "2");
  }

  #[test]
  fn block_height_endpoint() {
    let test_server = TestServer::new();

    let response = test_server.get("/blockheight");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().unwrap(), "0");

    test_server.mine_blocks(2);

    let response = test_server.get("/blockheight");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().unwrap(), "2");
  }

  #[test]
  fn block_hash_endpoint() {
    let test_server = TestServer::new();

    let response = test_server.get("/blockhash");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
      response.text().unwrap(),
      "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    );
  }

  #[test]
  fn block_hash_from_height_endpoint() {
    let test_server = TestServer::new();

    let response = test_server.get("/blockhash/0");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
      response.text().unwrap(),
      "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    );
  }

  #[test]
  fn block_time_endpoint() {
    let test_server = TestServer::new();

    let response = test_server.get("/blocktime");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().unwrap(), "1231006505");
  }

  #[test]
  fn block() {
    TestServer::new().assert_response_regex(
      "/block/0",
      StatusCode::OK,
      ".*<title>Block 0</title>.*<h1>Block 0</h1>.*",
    );
  }

  #[test]
  fn invalid_outpoint_hash_returns_400() {
    TestServer::new().assert_response(
      "/output/foo:0",
      StatusCode::BAD_REQUEST,
      "Invalid URL: error parsing TXID",
    );
  }

  #[test]
  fn output_without_sat_index() {
    let txid = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
    TestServer::new().assert_response_regex(
      format!("/output/{txid}:0"),
      StatusCode::OK,
      format!(
        ".*<title>Output {txid}:0</title>.*<h1>Output <span class=monospace>{txid}:0</span></h1>
<dl>
  <dt>value</dt><dd>5000000000</dd>
  <dt>script pubkey</dt><dd class=monospace>OP_PUSHBYTES_65 [[:xdigit:]]{{130}} OP_CHECKSIG</dd>
  <dt>transaction</dt><dd><a class=monospace href=/tx/{txid}>{txid}</a></dd>
  <dt>spent</dt><dd>false</dd>
</dl>.*"
      ),
    );
  }

  #[test]
  fn invalid_output_returns_400() {
    TestServer::new().assert_response(
      "/output/foo:0",
      StatusCode::BAD_REQUEST,
      "Invalid URL: error parsing TXID",
    );
  }

  #[test]
  fn blocks() {
    let test_server = TestServer::new();

    test_server.mine_blocks(1);

    test_server.assert_response_regex(
      "/blocks",
      StatusCode::OK,
      ".*<title>Blocks</title>.*
<h1>Blocks</h1>
<div class=block>
  <h2><a href=/block/1>Block 1</a></h2>
  <div class=thumbnails>
  </div>
</div>
<div class=block>
  <h2><a href=/block/0>Block 0</a></h2>
  <div class=thumbnails>
  </div>
</div>
</ol>.*",
    );
  }

  #[test]
  fn nav_displays_chain() {
    TestServer::builder()
      .chain(Chain::Regtest)
      .build()
      .assert_response_regex(
        "/",
        StatusCode::OK,
        ".*<a href=/ title=home>Ordinals<sup>regtest</sup></a>.*",
      );
  }

  #[test]
  fn blocks_block_limit() {
    let test_server = TestServer::new();

    test_server.mine_blocks(101);

    test_server.assert_response_regex(
      "/blocks",
      StatusCode::OK,
      ".*<ol start=96 reversed class=block-list>\n(  <li><a href=/block/[[:xdigit:]]{64}>[[:xdigit:]]{64}</a></li>\n){95}</ol>.*"
    );
  }

  #[test]
  fn block_not_found() {
    TestServer::new().assert_response(
      "/block/467a86f0642b1d284376d13a98ef58310caa49502b0f9a560ee222e0a122fe16",
      StatusCode::NOT_FOUND,
      "block 467a86f0642b1d284376d13a98ef58310caa49502b0f9a560ee222e0a122fe16 not found",
    );
  }

  #[test]
  fn static_asset() {
    TestServer::new().assert_response_regex(
      "/static/index.css",
      StatusCode::OK,
      r".*\.rare \{
  background-color: var\(--rare\);
}.*",
    );
  }

  #[test]
  fn favicon() {
    TestServer::new().assert_response_regex("/favicon.ico", StatusCode::OK, r".*");
  }

  #[test]
  fn clock_updates() {
    let test_server = TestServer::new();
    test_server.assert_response_regex("/clock", StatusCode::OK, ".*<text.*>0</text>.*");
    test_server.mine_blocks(1);
    test_server.assert_response_regex("/clock", StatusCode::OK, ".*<text.*>1</text>.*");
  }

  #[test]
  fn block_by_hash() {
    let test_server = TestServer::new();

    test_server.mine_blocks(1);
    let transaction = TransactionTemplate {
      inputs: &[(1, 0, 0, Default::default())],
      fee: 0,
      ..default()
    };
    test_server.core.broadcast_tx(transaction);
    let block_hash = test_server.mine_blocks(1)[0].block_hash();

    test_server.assert_response_regex(
      format!("/block/{block_hash}"),
      StatusCode::OK,
      ".*<h1>Block 2</h1>.*",
    );
  }

  #[test]
  fn block_by_height() {
    let test_server = TestServer::new();

    test_server.assert_response_regex("/block/0", StatusCode::OK, ".*<h1>Block 0</h1>.*");
  }

  #[test]
  fn transaction() {
    let test_server = TestServer::new();

    let coinbase_tx = test_server.mine_blocks(1)[0].txdata[0].clone();
    let txid = coinbase_tx.txid();

    test_server.assert_response_regex(
      format!("/tx/{txid}"),
      StatusCode::OK,
      format!(
        ".*<title>Transaction {txid}</title>.*<h1>Transaction <span class=monospace>{txid}</span></h1>
<dl>
</dl>
<h2>1 Input</h2>
<ul>
  <li><a class=monospace href=/output/0000000000000000000000000000000000000000000000000000000000000000:4294967295>0000000000000000000000000000000000000000000000000000000000000000:4294967295</a></li>
</ul>
<h2>1 Output</h2>
<ul class=monospace>
  <li>
    <a href=/output/{txid}:0 class=monospace>
      {txid}:0
    </a>
    <dl>
      <dt>value</dt><dd>5000000000</dd>
      <dt>script pubkey</dt><dd class=monospace>.*</dd>
    </dl>
  </li>
</ul>.*"
      ),
    );
  }

  #[test]
  fn detect_unrecoverable_reorg() {
    let test_server = TestServer::new();

    test_server.mine_blocks(21);

    test_server.assert_response_regex(
      "/status",
      StatusCode::OK,
      ".*<dt>unrecoverably reorged</dt>\n  <dd>false</dd>.*",
    );

    for _ in 0..15 {
      test_server.core.invalidate_tip();
    }

    test_server.core.mine_blocks(21);

    test_server.assert_response_regex(
      "/status",
      StatusCode::OK,
      ".*<dt>unrecoverably reorged</dt>\n  <dd>true</dd>.*",
    );
  }

  #[test]
  fn responses_are_gzipped() {
    let server = TestServer::new();

    let mut headers = HeaderMap::new();

    headers.insert(header::ACCEPT_ENCODING, "gzip".parse().unwrap());

    let response = reqwest::blocking::Client::builder()
      .default_headers(headers)
      .build()
      .unwrap()
      .get(server.join_url("/"))
      .send()
      .unwrap();

    assert_eq!(
      response.headers().get(header::CONTENT_ENCODING).unwrap(),
      "gzip"
    );
  }

  #[test]
  fn responses_are_brotlied() {
    let server = TestServer::new();

    let mut headers = HeaderMap::new();

    headers.insert(header::ACCEPT_ENCODING, "br".parse().unwrap());

    let response = reqwest::blocking::Client::builder()
      .default_headers(headers)
      .brotli(false)
      .build()
      .unwrap()
      .get(server.join_url("/"))
      .send()
      .unwrap();

    assert_eq!(
      response.headers().get(header::CONTENT_ENCODING).unwrap(),
      "br"
    );
  }
}
