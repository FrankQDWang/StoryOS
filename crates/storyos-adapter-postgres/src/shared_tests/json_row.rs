//! The JSON reader of the tests of this crate.

use crate::statement_cache::Client;
use serde_json::Value;
use tokio_postgres::types::ToSql;

/// The first column of the one row of the query `sql` with `params`, read as JSON text.
pub(crate) async fn json_of(admin: &Client, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Value {
    let row = admin.query_one(sql, params).await.unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}
