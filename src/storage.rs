use rusqlite::{Connection, Result, Transaction};

pub struct GraphStorage {
    conn: Connection,
}

impl GraphStorage {
    pub fn open(path: &str) -> Result<Self> {
        Ok(GraphStorage {
            conn: Connection::open(path)?,
        })
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS methods (
                    id   INTEGER PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    file_path TEXT 
            )",
            (),
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS calls (
                    caller_id INTEGER NOT NULL,
                    callee_id INTEGER NOT NULL,
                    PRIMARY KEY (caller_id, callee_id),
                    FOREIGN KEY (caller_id) REFERENCES methods(id),
                    FOREIGN KEY (callee_id) REFERENCES methods(id)
            )",
            (),
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee_id)",
            (),
        )?;

        Ok(())
    }
    fn retrieve_method_rowid(tx: &Transaction, method_name: &str) -> Result<i64> {
        tx.query_one(
            "SELECT id FROM methods WHERE name == ?",
            [method_name],
            |row| row.get(0),
        )
    }
    fn insert_method(tx: &Transaction, method_name: &str) -> Result<()> {
        tx.execute(
            "INSERT OR IGNORE INTO methods (name) VALUES (?)",
            [method_name],
        )?;
        Ok(())
    }
    fn insert_call(&mut self, caller: &str, callee: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        GraphStorage::insert_method(&tx, callee)?;
        GraphStorage::insert_method(&tx, caller)?;
        let caller_id = GraphStorage::retrieve_method_rowid(&tx, caller)?;
        let callee_id = GraphStorage::retrieve_method_rowid(&tx, callee)?;
        tx.execute(
            "INSERT OR IGNORE INTO calls (caller_id, callee_id) VALUES (?,?)",
            [caller_id, callee_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::storage::GraphStorage;

    #[test]
    fn test_creation() -> rusqlite::Result<()> {
        let storage = GraphStorage {
            conn: rusqlite::Connection::open_in_memory()?,
        };
        assert!(storage.init_schema().is_ok());
        Ok(())
    }

    #[test]
    fn test_insert_call() -> rusqlite::Result<()> {
        let mut storage = GraphStorage {
            conn: rusqlite::Connection::open_in_memory()?,
        };

        storage.init_schema()?;

        storage.insert_call("Service.save", "Repository.insert")?;
        storage.insert_call("Service.save", "Repository.insert")?;

        let count: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM calls", [], |row| row.get(0))?;

        assert_eq!(count, 1);
        Ok(())
    }
}
