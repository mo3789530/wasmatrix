pub mod conversion;
pub mod protocol;

#[cfg(test)]
mod protocol_tests;

pub use protocol::*;

pub mod v1 {
    tonic::include_proto!("wasmatrix.v1");
}
