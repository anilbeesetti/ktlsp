//! Native data-only consumer; no JVM, compiler, archive extraction or server.
use ktlsp_binary::{
    descriptors::{Component, FieldDescriptor, MethodDescriptor, Primitive, SlotContext},
    parse_class,
};

fn main() {
    let bytes = include_bytes!("../fixtures/t017c1/AppendableJoiner$Builder.class.bin");
    let class = parse_class(bytes).unwrap();
    assert_eq!((class.fields.len(), class.methods.len()), (4, 7));
    let mut parameters = 0;
    for field in &class.fields {
        let raw = class.utf16(field.descriptor).unwrap();
        let view = FieldDescriptor::parse(raw).unwrap();
        assert!(std::ptr::eq(view.raw(), raw));
        assert_eq!(view.dimensions(), 0);
        assert!(matches!(view.component(), Component::Object(_)));
    }
    for member in &class.methods {
        let raw = class.utf16(member.descriptor).unwrap();
        let view = MethodDescriptor::parse(raw, SlotContext::WithReceiver).unwrap();
        assert!(std::ptr::eq(view.raw(), raw));
        parameters += view.parameters().count();
    }
    assert_eq!(parameters, 4);
    let raw = [40, 74, 91, 91, 68, 41, 86]; // (J[[D)V
    let view = MethodDescriptor::parse(&raw, SlotContext::WithReceiver).unwrap();
    assert_eq!(view.parameter_slots(), 4);
    assert_eq!(view.result(), None);
    let array = view.parameters().nth(1).unwrap();
    assert_eq!(array.dimensions(), 2);
    assert_eq!(array.component(), Component::Primitive(Primitive::Double));
    assert!(std::ptr::eq(array.raw(), &raw[2..5]));
    assert!(MethodDescriptor::parse(&raw[..6], SlotContext::WithReceiver).is_err());
    println!(
        "descriptor smoke: retained Commons Lang Builder, 4 fields, 7 methods, 4 parameters; borrowed primitive/array/void and malformed data passed"
    );
}
