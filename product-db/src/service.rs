use std::sync::Arc;

use axum::Router;
use log::{error, info, warn};
use tokio::sync::watch;

use crate::{DataBackend, Error, Options, Result};

/// The central service that provides access to the product database.
pub struct Service<DB: DataBackend> {
    options: Options,
    db: Arc<DB>,
    stop_signal_receiver: watch::Receiver<i32>,
    stop_signal_sender: watch::Sender<i32>,
}

impl<DB: DataBackend> Service<DB> {
    /// Creates a new instance of the service.
    ///
    /// # Arguments
    /// - `options` - The options for the service.
    pub async fn new(options: Options) -> Result<Self> {
        // create postgres database instance
        let db = Arc::new(DB::new(&options).await?);

        // create the stop signal channel with the initial value set to running=false
        let (tx, rx) = watch::channel(0);

        Ok(Self {
            options,
            db,
            stop_signal_receiver: rx,
            stop_signal_sender: tx,
        })
    }

    /// Returns the router for the service.
    pub async fn run(&self) -> Result<()> {
        let app = Self::setup_routes(Router::new(), self.db.clone());

        let rx = self.stop_signal_receiver.clone();

        // create the listener on the given address
        info!("Start listening on '{}'...", self.options.address);
        let listener = match tokio::net::TcpListener::bind(self.options.address.as_str()).await {
            Ok(listener) => listener,
            Err(e) => {
                error!("Start listening on '{}'...FAILED", self.options.address);
                error!(
                    "Failed to bind to the address {} due to {}",
                    self.options.address, e
                );
                return Err(Error::NetworkError(e));
            }
        };

        info!("Start listening on '{}'...OK", self.options.address);

        // start the server...
        info!("Starting the server...");
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = rx.clone();
                // wait for the signal to shutdown the server
                if let Err(err) = rx.changed().await {
                    warn!("Failed to receive the stop signal: {}", err);
                    return;
                }

                info!("Received stop signal, stopping the server...");
            })
            .await
            .map_err(|e| {
                error!("Server error: {}", e);
                Error::NetworkError(e)
            })?;

        info!("Server stopped.");

        Ok(())
    }

    /// Stops the service.
    pub fn stop(&self) {
        info!("Stopping the server...");
        if let Err(err) = self.stop_signal_sender.send(1) {
            error!("Failed to send the stop signal: {}", err);
        }
    }

    /// Sets up the routes for the service and returns the app.
    ///
    /// # Arguments
    /// - `app` - The app to set up the routes on.
    /// - `db` - The data backend instance to use.
    fn setup_routes(app: Router, db: Arc<DB>) -> Router {
        app
    }
}
