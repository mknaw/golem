use anyhow::anyhow;
use prost_types::DescriptorProto;
use semver::Version;
use wit_encoder::{Interface, Package, PackageName};

const TYPES_INTERFACE: &str = "types";

pub fn grpc_to_wit() -> anyhow::Result<Package> {
    let source = r#"
syntax = "proto3";
package core.todo.v1;

option go_package = "github.com/koblas/grpc-todo/protos";

message TodoListRequest { string user_id = 1; }

message TodoAddRequest {
  string user_id = 1;
  string task = 2;
}

message TodoDeleteRequest {
  string user_id = 1;
  string id = 2;
}

message TodoObject {
  string user_id = 1;
  string id = 2;
  string task = 3;
}

message TodoChangeEvent {
  string idemponcy_id = 1;
  TodoObject current = 3;
  TodoObject original = 4;
}

message TodoAddResponse { TodoObject todo = 1; }
message TodoListResponse { repeated TodoObject todos = 1; }
message TodoDeleteResponse { string message = 1; }

service TodoService {
  rpc TodoAdd(TodoAddRequest) returns (TodoAddResponse);
  rpc TodoDelete(TodoDeleteRequest) returns (TodoDeleteResponse);
  rpc TodoList(TodoListRequest) returns (TodoListResponse);
}
    "#;
    let parsed = protox_parse::parse("todo.proto", source).map_err(|e| anyhow!(e))?;
    let grpc_package = parsed.package.ok_or_else(|| anyhow!("package not found"))?;

    let package_name = extract_package_name(&grpc_package)
        .ok_or_else(|| anyhow!("failed to extract package name from {:?}", grpc_package))?;

    let mut types_interface = Interface::new(TYPES_INTERFACE);

    for message in parsed.message_type {
        add_message_type(&mut types_interface, &message);
    }

    let mut package = Package::new(package_name);

    todo!();
}

fn extract_package_name(grpc_package: &str) -> Option<PackageName> {
    Some(PackageName::new(
        "core",
        "todo",
        extract_version(grpc_package),
    ))
}

fn extract_version(grpc_package: &str) -> Option<Version> {
    // TODO
    return Some(Version::new(1, 0, 0));
}

fn add_message_type(
    interface: &mut Interface,
    message_type: &DescriptorProto,
) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn primitive() {
        assert_eq!(1 + 3, 2);
    }
}
