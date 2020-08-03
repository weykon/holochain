//! Various types for the databases involved in the DhtOp integration workflow

use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_p2p::dht_arc::DhtArc;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvBuf,
    db::INTEGRATED_DHT_OPS,
    error::{DatabaseError, DatabaseResult},
    prelude::{BufferedStore, GetDb, Reader},
};
use holochain_types::{
    dht_op::{DhtOp, DhtOpLight},
    validate::ValidationStatus,
    Timestamp,
};

/// Database type for AuthoredDhtOps
/// Buffer for accessing [DhtOp]s that you authored and finding the amount of validation receipts
pub type AuthoredDhtOpsStore<'env> =
    KvBuf<'env, AuthoredDhtOpsKey, AuthoredDhtOpsValue, Reader<'env>>;

/// The key type for the AuthoredDhtOps db: a DhtOpHash
pub type AuthoredDhtOpsKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuthoredDhtOpsValue {
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Validation receipts received
    pub receipt_count: u32,
    /// Time last published, None if never published
    pub last_publish_time: Option<Timestamp>,
}

impl AuthoredDhtOpsValue {
    /// Create a new value from a DhtOpLight with no receipts and no timestamp
    pub fn from_light(op: DhtOpLight) -> Self {
        Self {
            op,
            receipt_count: 0,
            last_publish_time: None,
        }
    }
}

/// Database type for IntegrationQueue: the queue of ops ready to be integrated.
/// NB: this is not really a queue because it doesn't envorce FIFO.
/// We should probably change the name.
pub type IntegrationQueueStore<'env> =
    KvBuf<'env, IntegrationQueueKey, IntegrationQueueValue, Reader<'env>>;

/// Database type for IntegratedDhtOps
/// [DhtOp]s that have already been integrated
pub type IntegratedDhtOpsStore<'env> = KvBuf<'env, DhtOpHash, IntegratedDhtOpsValue, Reader<'env>>;

/// Buffer that adds query logic to the IntegratedDhtOpsStore
pub struct IntegratedDhtOpsBuf<'env> {
    store: IntegratedDhtOpsStore<'env>,
}

impl<'env> std::ops::Deref for IntegratedDhtOpsBuf<'env> {
    type Target = IntegratedDhtOpsStore<'env>;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl<'env> std::ops::DerefMut for IntegratedDhtOpsBuf<'env> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

impl<'env> BufferedStore<'env> for IntegratedDhtOpsBuf<'env> {
    type Error = DatabaseError;
    fn flush_to_txn(
        self,
        writer: &'env mut holochain_state::prelude::Writer,
    ) -> Result<(), Self::Error> {
        self.store.flush_to_txn(writer)
    }
}

/// The key type for the IntegrationQueue db is just a DhtOpHash
pub type IntegrationQueueKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegratedDhtOpsValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
    /// Time when the op was integrated
    pub when_integrated: Timestamp,
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct IntegrationQueueValue {
    /// The op's validation status
    pub validation_status: ValidationStatus,
    /// The op
    pub op: DhtOp,
}

impl<'env> IntegratedDhtOpsBuf<'env> {
    /// Create a new buffer for the IntegratedDhtOpsStore
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS).unwrap();
        Ok(Self {
            store: IntegratedDhtOpsStore::new(&reader, db)?,
        })
    }

    /// simple get by dht_op_hash
    pub fn get(&'_ self, op_hash: &DhtOpHash) -> DatabaseResult<Option<IntegratedDhtOpsValue>> {
        self.store.get(op_hash)
    }

    /// Get ops that match optional queries:
    /// - from a time (Inclusive)
    /// - to a time (Exclusive)
    /// - match a dht location
    pub fn query(
        &'env self,
        from: Option<Timestamp>,
        to: Option<Timestamp>,
        dht_arc: Option<DhtArc>,
    ) -> DatabaseResult<
        Box<
            dyn FallibleIterator<Item = (DhtOpHash, IntegratedDhtOpsValue), Error = DatabaseError>
                + 'env,
        >,
    > {
        Ok(Box::new(
            self.store
                .iter()?
                .map(move |(k, v)| Ok((DhtOpHash::with_pre_hashed(k.to_vec()), v)))
                .filter_map(move |(k, v)| match from {
                    Some(time) if v.when_integrated >= time => Ok(Some((k, v))),
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                })
                .filter_map(move |(k, v)| match to {
                    Some(time) if v.when_integrated < time => Ok(Some((k, v))),
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                })
                .filter_map(move |(k, v)| match dht_arc {
                    Some(dht_arc) if dht_arc.contains(v.op.dht_basis().get_loc()) => {
                        Ok(Some((k, v)))
                    }
                    None => Ok(Some((k, v))),
                    _ => Ok(None),
                }),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::AnyDhtHashFixturator;
    use ::fixt::prelude::*;
    use chrono::{Duration, Utc};
    use holo_hash::fixt::{DhtOpHashFixturator, HeaderHashFixturator};
    use holochain_state::test_utils::test_cell_env;
    use holochain_state::{
        buffer::BufferedStore,
        env::{ReadManager, WriteManager},
    };
    use pretty_assertions::assert_eq;

    #[tokio::test(threaded_scheduler)]
    async fn test_query() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Create some integration values
        let mut expected = Vec::new();
        let mut basis = AnyDhtHashFixturator::new(Predictable);
        let now = Utc::now();
        let same_basis = basis.next().unwrap();
        let mut times = Vec::new();
        times.push(now - Duration::hours(100));
        times.push(now);
        times.push(now + Duration::hours(100));
        let times_exp = times.clone();
        let values = times
            .into_iter()
            .map(|when_integrated| IntegratedDhtOpsValue {
                validation_status: ValidationStatus::Valid,
                op: DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), basis.next().unwrap()),
                when_integrated: when_integrated.into(),
            });

        // Put them in the db
        {
            let mut dht_hash = DhtOpHashFixturator::new(Predictable);
            let reader = env_ref.reader().unwrap();
            let mut buf = IntegratedDhtOpsBuf::new(&reader, &dbs).unwrap();
            for mut value in values {
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
                value.op = DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), same_basis.clone());
                buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
                expected.push(value.clone());
            }
            env_ref
                .with_commit(|writer| buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check queries
        {
            let reader = env_ref.reader().unwrap();
            let buf = IntegratedDhtOpsBuf::new(&reader, &dbs).unwrap();
            // No filter
            let mut r = buf
                .query(None, None, None)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert_eq!(&r[..], &expected[..]);
            // From now
            let mut r = buf
                .query(Some(times_exp[1].clone().into()), None, None)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[2]));
            assert!(r.contains(&expected[4]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 4);
            // From ages ago till 1hr in future
            let ages_ago = times_exp[0] - Duration::weeks(5);
            let future = times_exp[1] + Duration::hours(1);
            let mut r = buf
                .query(Some(ages_ago.into()), Some(future.into()), None)
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());

            assert!(r.contains(&expected[0]));
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[2]));
            assert!(r.contains(&expected[3]));
            assert_eq!(r.len(), 4);
            // Same basis
            let ages_ago = times_exp[0] - Duration::weeks(5);
            let future = times_exp[1] + Duration::hours(1);
            let mut r = buf
                .query(
                    Some(ages_ago.into()),
                    Some(future.into()),
                    Some(DhtArc::new(same_basis.get_loc(), 1)),
                )
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert_eq!(r.len(), 2);
            // Same basis all
            let mut r = buf
                .query(None, None, Some(DhtArc::new(same_basis.get_loc(), 1)))
                .unwrap()
                .map(|(_, v)| Ok(v))
                .collect::<Vec<_>>()
                .unwrap();
            r.sort_by_key(|v| v.when_integrated.clone());
            assert!(r.contains(&expected[1]));
            assert!(r.contains(&expected[3]));
            assert!(r.contains(&expected[5]));
            assert_eq!(r.len(), 3);
        }
    }
}
