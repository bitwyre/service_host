#[macro_use]
extern crate log;

use futures::{future::Future, stream::Stream};
use signal_hook::{self, iterator::Signals};
use std::{io::Error, marker::PhantomData, sync::Arc};
use tokio;
use tokio_threadpool::{Builder, ThreadPool};

pub trait ServiceControl<T> {
    fn create_and_start(data: Arc<&'static T>, thread_pool: &ThreadPool) -> Self;
    fn shutdown(self);
}

pub struct ServiceHost<T, S> {
    service_container: S,
    thread_manager: ThreadPool,
    phantom: PhantomData<T>,
}

impl<T, S> ServiceHost<T, S>
where
    T: Send + 'static,
    S: ServiceControl<T> + Send + 'static,
{
    pub fn create_and_start(worker_count: u16, data: Arc<&'static T>) -> Self {
        let thread_pool = Builder::new().pool_size(worker_count as usize).build();
        Self {
            service_container: S::create_and_start(data, &thread_pool),
            thread_manager: thread_pool,
            phantom: PhantomData,
        }
    }
}

impl<T, S> ServiceHost<T, S>
where
    T: Send + 'static,
    S: ServiceControl<T> + Send + 'static,
{
    pub fn wait_for_signal(self) -> Result<(), Error> {
        let wait_signal = Signals::new(&[signal_hook::SIGINT, signal_hook::SIGTERM])?
            .into_async()?
            .into_future()
            .map(move |_| {
                info!("Shutdown sequence initiated...");
                self.service_container.shutdown();
                let _ = self.thread_manager.shutdown().catch_unwind().wait();
            })
            .map_err(|e| {
                let error_message = format!("Signal panic!\n{}", e.0);
                error!("{}", &error_message);
                panic!("{}", &error_message);
            });
        info!("Waiting for exit signal...");
        tokio::run(wait_signal);
        Ok(())
    }
}
