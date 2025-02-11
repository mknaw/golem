use std::collections::HashMap;
use std::convert::TryFrom;

use anyhow::anyhow;
use prost_types::field_descriptor_proto::{Label, Type as ProtobufType};
use prost_types::{DescriptorProto, FieldDescriptorProto};
use semver::Version;
use wit_encoder::{
    Interface, InterfaceItem, Package, PackageItem, PackageName, StandaloneFunc, Type as WitType,
    TypeDef as WitTypeDef, TypeDefKind as WitTypeDefKind, World,
};

const TYPES_INTERFACE: &str = "types";

pub fn grpc_to_wit(source: &str) -> anyhow::Result<Package> {
    // TODO have to figure out how to pass it a name (`"TODO.proto"`)
    let parsed = protox_parse::parse("TODO.proto", source).map_err(|e| anyhow!(e))?;
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

    // TODO this may be a little wasteful...
    let type_defs: HashMap<String, WitTypeDef> = types_interface
        .items()
        .iter()
        .filter_map(|item| {
            if let InterfaceItem::TypeDef(type_def) = item {
                Some(type_def)
            } else {
                None
            }
        })
        .map(|type_def| (type_def.name().as_ref().to_string(), type_def.clone()))
        .collect();

    let mut package = Package::new(package_name.clone());
    package.interface(types_interface);

    for service in parsed.service {
        let name = service
            .name
            .as_deref()
            .ok_or_else(|| anyhow!("service name not found"))
            .map(title_to_kebab)?;
        let mut service_interface = Interface::new(name);

        for type_def in type_defs.values() {
            service_interface.use_type("types", type_def.name().clone(), None);
        }

        for method in service.method {
            let func_name = method
                .name
                .as_deref()
                .ok_or_else(|| anyhow!("method name not found"))
                .map(title_to_kebab)?;

            let mut func = StandaloneFunc::new(func_name);

            let var_name = method
                .input_type
                .as_deref()
                .ok_or_else(|| anyhow!("method input_type not found"))
                .map(title_to_kebab)?;

            // TODO what's the difference between `TypeDef` and `Type`?
            let wit_type_def = type_defs
                .get(&var_name)
                .ok_or_else(|| anyhow!("type not found: {}", var_name))?;

            match wit_type_def.kind() {
                WitTypeDefKind::Type(wit_type) => {
                    func.set_params((var_name, wit_type.clone()));
                }
                // TODO not sure if this is appropriate for all cases!
                _ => func.set_params((var_name.clone(), WitType::Named(var_name.into()))),
            }

            service_interface.function(func);
        }

        package.interface(service_interface);
    }

    let mut world = World::new(package_name.name().clone());
    for item in package.items().iter() {
        match item {
            PackageItem::Interface(interface) => {
                world.named_interface_export(interface.name().clone());
            }
            _ => unreachable!(),
        }
    }
    package.world(world);

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

fn proto_field_to_wit_type(field: &FieldDescriptorProto) -> anyhow::Result<WitType> {
    // TODO (not a big deal) is there some cleaner way to not have intermediate var?
    let res: anyhow::Result<WitType> = match field.r#type {
        Some(variant) => {
            let protobuf_type = ProtobufType::try_from(variant)?;
            let wit_type = match protobuf_type {
                ProtobufType::Bool => WitType::Bool,
                ProtobufType::Uint32 => WitType::U32,
                ProtobufType::Uint64 => WitType::U64,
                ProtobufType::Int32 => WitType::S32,
                ProtobufType::Int64 => WitType::S64,
                ProtobufType::Float => WitType::F32,
                ProtobufType::Double => WitType::F64,
                ProtobufType::String => WitType::String,
                ProtobufType::Fixed64 => todo!(),
                ProtobufType::Fixed32 => todo!(),
                ProtobufType::Group => todo!(),
                ProtobufType::Message => todo!(),
                ProtobufType::Bytes => todo!(),
                ProtobufType::Enum => todo!(),
                ProtobufType::Sfixed32 => todo!(),
                ProtobufType::Sfixed64 => todo!(),
                ProtobufType::Sint32 => todo!(),
                ProtobufType::Sint64 => todo!(),
            };
            return Ok(wit_type);
        }
        None => match field.type_name.as_ref() {
            Some(name) => Ok(WitType::Named(title_to_kebab(name).into())),
            None => Err(anyhow!("neither type nor field_type defined")),
        },
    };
    let primitive = res?;
    let label = field.label.map(Label::try_from).transpose()?;
    // TODO do we have to do anything fancy with the `Label::Required`?
    // TODO maybe the "optional everything" thing that gRPC does needs special consideration..
    // in fact I think we should be saying `Option` if it's a `proto3_optional`.
    let wit_type = match label {
        Some(Label::Repeated) => WitType::List(Box::new(primitive)),
        Some(Label::Optional) => WitType::Option(Box::new(primitive)),
        _ => primitive,
    };

    Ok(wit_type)
}

// TODO OK to rely on the input following these conventions?
fn snake_to_kebab(input: &str) -> String {
    input.replace("_", "-")
}

fn title_to_kebab(title: &str) -> String {
    let mut kebab = String::new();
    let mut prev_char = ' ';

    for c in title.chars() {
        if c.is_uppercase() {
            if prev_char != ' ' {
                kebab.push('-');
            }
            kebab.push(c.to_lowercase().next().unwrap());
        } else {
            kebab.push(c);
        }
        prev_char = c;
    }

    kebab
}

fn add_message_type(
    interface: &mut Interface,
    message_type: &DescriptorProto,
) -> anyhow::Result<()> {
    let name = message_type
        .name
        .as_deref()
        .ok_or_else(|| anyhow!("message type name not found"))
        .map(title_to_kebab)?;

    let fields = message_type
        .field
        .iter()
        .map(|field| {
            let field_name = field
                .name
                .as_deref()
                .ok_or_else(|| anyhow!("field name not found"))
                .map(snake_to_kebab)?;

            let field_type = proto_field_to_wit_type(field)?;
            Ok((snake_to_kebab(&field_name), field_type))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    interface.type_def(WitTypeDef::record(name, fields));

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
        let package = super::grpc_to_wit(SOURCE).unwrap_or_else(|e| panic!("{}", e));
        println!("{}", package.to_string());
        assert_eq!(package.to_string(), EXPECTED);
    }
}
