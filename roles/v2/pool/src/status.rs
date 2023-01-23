use crate::error::PoolError;

#[derive(Debug)]
pub enum Sender {
    Downstream(async_channel::Sender<Status>),
    DownstreamListener(async_channel::Sender<Status>),
    Upstream(async_channel::Sender<Status>),
}

impl Sender {
    pub fn listener_to_connection(&self) -> Self {
        match self {
            Self::DownstreamListener(inner) => Self::Downstream(inner.clone()),
            _ => unreachable!(),
        }
    }

    pub async fn send(&self, status: Status) -> Result<(), async_channel::SendError<Status>> {
        match self {
            Self::Downstream(inner) => inner.send(status).await,
            Self::DownstreamListener(inner) => inner.send(status).await,
            Self::Upstream(inner) => inner.send(status).await,
        }
    }
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        match self {
            Self::Downstream(inner) => Self::Downstream(inner.clone()),
            Self::DownstreamListener(inner) => Self::DownstreamListener(inner.clone()),
            Self::Upstream(inner) => Self::Upstream(inner.clone()),
        }
    }
}

#[derive(Debug)]
pub enum State {
    DownstreamShutdown(PoolError),
    TemplateProviderShutdown(PoolError),
    Healthy(String),
}

#[derive(Debug)]
pub struct Status {
    pub state: State,
}

async fn send_status(
    sender: &Sender,
    e: PoolError,
    outcome: error_handling::ErrorBranch,
) -> error_handling::ErrorBranch {
    match sender {
        Sender::Downstream(tx) => {
            let string_err = e.to_string();
            tx.send(Status {
                state: State::Healthy(string_err),
            })
            .await
            .unwrap_or(());
        }
        Sender::DownstreamListener(tx) => {
            tx.send(Status {
                state: State::DownstreamShutdown(e),
            })
            .await
            .unwrap_or(());
        }
        Sender::Upstream(tx) => {
            tx.send(Status {
                state: State::TemplateProviderShutdown(e),
            })
            .await
            .unwrap_or(());
        }
    }
    outcome
}

// this is called by `error_handling::handle_result!`
pub async fn handle_error(
    sender: &Sender,
    e: crate::error::PoolError,
) -> error_handling::ErrorBranch {
    tracing::debug!("Error: {:?}", &e);
    match e {
        PoolError::Io(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        PoolError::ChannelSend(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Break).await
        }
        PoolError::ChannelRecv(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Break).await
        }
        PoolError::BinarySv2(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        PoolError::Codec(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        PoolError::Noise(_) => send_status(sender, e, error_handling::ErrorBranch::Continue).await,
        PoolError::RolesLogic(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Break).await
        }
        PoolError::Framing(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        PoolError::PoisonLock(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Break).await
        }
    }
}