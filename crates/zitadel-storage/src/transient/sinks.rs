use tokio::{
    sync::mpsc,
    time::{self, Duration},
};
use zitadel_db::Db;

use super::{
    Sink, TransientRecord, apply_channel_batch, drain_sink_inbox, ensure_sink_inbox_table,
    insert_sink_record,
};

#[derive(Clone, Default)]
pub struct NoopSink;

impl Sink for NoopSink {
    async fn emit(&self, _record: TransientRecord) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ChannelSink {
    tx: mpsc::Sender<TransientRecord>,
}

impl ChannelSink {
    pub fn new(db: Db, buffer_size: usize, batch_size: usize, flush_interval: Duration) -> Self {
        let (tx, mut rx) = mpsc::channel(buffer_size.max(1));
        let db_clone = db.clone();
        tokio::spawn(async move {
            let mut ticker = time::interval(flush_interval);
            let mut pending = Vec::new();
            loop {
                tokio::select! {
                    maybe_record = rx.recv() => {
                        match maybe_record {
                            Some(record) => {
                                pending.push(record);
                                if pending.len() >= batch_size.max(1)
                                    && apply_channel_batch(&db_clone, &mut pending).await.is_err()
                                {
                                    ticker.tick().await;
                                }
                            }
                            None => {
                                let _ = apply_channel_batch(&db_clone, &mut pending).await;
                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let _ = apply_channel_batch(&db_clone, &mut pending).await;
                    }
                }
            }
        });
        Self { tx }
    }
}

impl Sink for ChannelSink {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
        self.tx.send(record).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SqlSink {
    buffer_db: Db,
    target_db: Db,
    batch_size: usize,
}

impl SqlSink {
    pub async fn new(
        buffer_db: Db,
        target_db: Db,
        batch_size: usize,
        flush_interval: Duration,
    ) -> anyhow::Result<Self> {
        ensure_sink_inbox_table(&buffer_db).await?;
        let buffer_db_clone = buffer_db.clone();
        let target_db_clone = target_db.clone();
        tokio::spawn(async move {
            let mut ticker = time::interval(flush_interval);
            loop {
                ticker.tick().await;
                if let Err(error) =
                    drain_sink_inbox(&buffer_db_clone, &target_db_clone, batch_size.max(1)).await
                {
                    tracing::warn!(stream = "event_pusher", %error, "sql sink drain failed");
                }
            }
        });
        Ok(Self {
            buffer_db,
            target_db,
            batch_size: batch_size.max(1),
        })
    }

    pub async fn drain_once(&self) -> anyhow::Result<()> {
        drain_sink_inbox(&self.buffer_db, &self.target_db, self.batch_size).await
    }
}

impl Sink for SqlSink {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
        insert_sink_record(&self.buffer_db, &record).await
    }
}
