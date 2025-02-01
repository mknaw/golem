// use std::iter;
use std::convert::TryFrom;

use anyhow::anyhow;
use prost_types::field_descriptor_proto::Label;
use prost_types::{DescriptorProto, FieldDescriptorProto};
use semver::Version;
use wit_encoder::{Interface, Package, PackageName, Type, TypeDef};

const TYPES_INTERFACE: &str = "types";

// TODO have to try a recursive type
//
// oneof         -> variant
// message       -> record
// string        -> string
// int32         -> s32
// int64         -> s64
// uint32        -> u32
// uint64        -> u64
// float         -> float32
// double        -> float64
// bool/boolean  -> bool
// repeated T    -> list<T>
// optional T    -> option<T>

pub fn grpc_to_wit(source: &str) -> anyhow::Result<Package> {
    let parsed = protox_parse::parse("todo.proto", source).map_err(|e| anyhow!(e))?;
    let grpc_package = parsed
        .package
        .as_ref()
        .ok_or_else(|| anyhow!("package not found"))?;

    let package_name = extract_package_name(grpc_package)
        .ok_or_else(|| anyhow!("failed to extract package name from {:?}", grpc_package))?;

    let mut types_interface = Interface::new(TYPES_INTERFACE);

    for message in parsed.message_type {
        add_message_type(&mut types_interface, &message)?;
    }

    // for service in parsed.service {}

    let mut package = Package::new(package_name);
    package.interface(types_interface);

    Ok(package)
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

// TODO I could take it or leave it with this layout of the two fns
fn proto_type_name_to_wit_primitive(type_name: &str) -> anyhow::Result<Type> {
    let wit_type = match type_name {
        "string" => Type::String,
        "int32" => Type::S32,
        "int64" => Type::S64,
        "uint32" => Type::U32,
        "uint64" => Type::U64,
        "float" => Type::F32,
        "double" => Type::F64,
        "bool" => Type::Bool,
        // TODO repeated T
        // TODO optional T
        name => Type::Named(name.to_owned().into()),
    };
    Ok(wit_type)
}

// TODO error handling
fn proto_field_to_wit_type(field: &FieldDescriptorProto) -> anyhow::Result<Type> {
    let type_name = field
        .type_name
        .as_ref()
        .ok_or_else(|| anyhow!("field type name not found for field {:?}", field))?;
    let primitive = proto_type_name_to_wit_primitive(type_name)?;
    let label = field.label.map(Label::try_from).transpose()?;
    // TODO do we have to do anything fancy with the `Label::Required`?
    let wit_type = match label {
        Some(Label::Repeated) => Type::List(Box::new(primitive)),
        Some(Label::Optional) => Type::Option(Box::new(primitive)),
        _ => primitive,
    };

    Ok(wit_type)
}

fn add_message_type(
    interface: &mut Interface,
    message_type: &DescriptorProto,
) -> anyhow::Result<()> {
    let name = message_type
        .name
        .clone()
        .ok_or_else(|| anyhow!("message type name not found"))?;

    let fields = message_type
        .field
        .iter()
        .map(|field| {
            let field_name = field
                .name
                .clone()
                .ok_or_else(|| anyhow!("field name not found"))?;

            let field_type = proto_field_to_wit_type(field)?;
            Ok((field_name, field_type))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    interface.type_def(TypeDef::record(name, fields));

    Ok(())
}

#[cfg(test)]
mod tests {
    use test_r::test;

    const SOURCE: &str = r#"
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

    const EXPECTED: &str = r#"
package core:todo@1.0.0;

interface types {
    record todo-object {
        user-id: string,
        id: string,
        task: string,
    }

    record todo-change-event {
        idempotency-id: string,
        current: todo-object,
        original: todo-object,
    }

    record todo-list-request {
        user-id: string,
    }

    record todo-add-request {
        user-id: string,
        task: string,
    }

    record todo-delete-request {
        user-id: string,
        id: string,
    }

    record todo-add-response {
        todo: todo-object,
    }

    record todo-list-response {
        todos: list<todo-object>,
    }

    record todo-delete-response {
        message: string,
    }

    variant todo-error {
        not-found,
        unauthorized,
        invalid-input,
        internal-error,
    }
}

interface todo-service {
    use types.{
        todo-add-request, 
        todo-add-response,
        todo-delete-request,
        todo-delete-response,
        todo-list-request,
        todo-list-response,
        todo-error,
    };

    todo-add: func(request: todo-add-request) -> result<todo-add-response, todo-error>;
    todo-delete: func(request: todo-delete-request) -> result<todo-delete-response, todo-error>;
    todo-list: func(request: todo-list-request) -> result<todo-list-response, todo-error>;
}
    "#;

    #[test]
    pub fn grpc_to_wit() {
        let package = super::grpc_to_wit(SOURCE).unwrap();
        assert_eq!(package.to_string(), EXPECTED);
    }
}
