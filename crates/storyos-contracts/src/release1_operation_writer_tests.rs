use super::super::{generated_files, generated_files_of};
use super::table_source;

#[test]
fn table_writer_emits_the_bytes_of_the_module_writer() {
    let modules = generated_files();
    let table = generated_files_of(table_source());
    for index in 0..modules.len().max(table.len()) {
        let (module_path, module_bytes) = modules
            .get(index)
            .map_or(("none", &[][..]), |file| (file.0, &file.1));
        let (table_path, table_bytes) = table
            .get(index)
            .map_or(("none", &[][..]), |file| (file.0, &file.1));
        assert_eq!(
            module_path, table_path,
            "file {index} of the two writers differs"
        );
        if let Some(offset) = first_difference(module_bytes, table_bytes) {
            panic!("{table_path} differs first at byte offset {offset}");
        }
    }
}

fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))
}
