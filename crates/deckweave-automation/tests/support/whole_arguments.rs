// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_automation::{ActionArguments, ValueBinding};
use deckweave_domain::{capability::NumericRange, schema::PayloadSchema};
use serde_json::{Value, json};

fn fixture(
    shape: Value,
    value: Value,
) -> Result<(GraphDocument, GraphPlanCompiler), Box<dyn Error>> {
    let (mut graph, compiler) = argument_compiler::binding_fixture()?;
    graph.schema_version.minor = 1;
    let args = graph
        .graph
        .action_arguments
        .get_mut(&GraphNodeId::try_new("action")?)
        .ok_or("args")?;
    args.fields.clear();
    args.value = Some(ValueBinding::Constant { value });
    let mut schemas = compiler.catalog().schemas().to_vec();
    let input = schemas
        .iter_mut()
        .find(|s| s.schema.id.as_str() == "org.deckweave.schema.task")
        .ok_or("schema")?;
    input.value_kind = SchemaValueKind::Structured(serde_json::from_value(shape)?);
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(schemas, compiler.catalog().endpoints().to_vec())?,
        GraphLimits::default(),
    )?;
    Ok((graph, compiler))
}
fn compiles(graph: &GraphDocument, compiler: &GraphPlanCompiler) -> Result<bool, Box<dyn Error>> {
    Ok(compiler
        .compile_document(
            &serde_json::to_value(graph)?,
            &package_lock()?,
            "lock",
            &fence()?,
        )
        .is_ok())
}
fn args(graph: &mut GraphDocument) -> Result<&mut ActionArguments, Box<dyn Error>> {
    graph
        .graph
        .action_arguments
        .values_mut()
        .next()
        .ok_or_else(|| "args".into())
}

#[test]
fn scalar_array_nullable_and_record_inputs_validate_before_dispatch() -> Result<(), Box<dyn Error>>
{
    for (shape, good, bad) in [
        (
            json!({"type":"number","minimum":0,"maximum":1}),
            json!(0.4),
            json!(1.01),
        ),
        (json!({"type":"boolean"}), json!(true), json!("true")),
        (
            json!({"type":"string","maximumBytes":10,"choices":["next","previous"]}),
            json!("next"),
            json!("typo"),
        ),
        (
            json!({"type":"array","maximumItems":2,"items":{"type":"boolean"}}),
            json!([true, false]),
            json!([true, false, true]),
        ),
        (
            json!({"type":"nullable","inner":{"type":"boolean"}}),
            json!(null),
            json!(1),
        ),
        (
            json!({"type":"record","fields":{"on":{"schema":{"type":"boolean"}}}}),
            json!({"on":false}),
            json!({}),
        ),
        (json!({"type":"null"}), json!(null), json!(false)),
    ] {
        let (mut graph, compiler) = fixture(shape.clone(), good.clone())?;
        assert!(compiles(&graph, &compiler)?, "{shape}");
        assert_eq!(
            args(&mut graph)?.assemble(&json!("trigger"), &BTreeMap::new(), &BTreeMap::new()),
            Some(good)
        );
        args(&mut graph)?.value = Some(ValueBinding::Constant { value: bad });
        assert!(!compiles(&graph, &compiler)?, "{shape}");
    }
    Ok(())
}

#[test]
fn whole_input_requires_new_format_and_cannot_mix_with_named_fields() -> Result<(), Box<dyn Error>>
{
    let (mut graph, compiler) = fixture(json!({"type":"boolean"}), json!(true))?;
    graph.schema_version.minor = 0;
    assert!(!compiles(&graph, &compiler)?);
    graph.schema_version.minor = 1;
    args(&mut graph)?.fields.insert(
        "ignored".into(),
        ValueBinding::Constant {
            value: json!(false),
        },
    );
    assert!(!compiles(&graph, &compiler)?);
    assert_eq!(
        args(&mut graph)?.assemble(&Value::Null, &BTreeMap::new(), &BTreeMap::new()),
        None
    );
    Ok(())
}

#[test]
fn whole_input_pointer_cannot_read_flow_payload_or_future_result() -> Result<(), Box<dyn Error>> {
    let (mut graph, compiler) = fixture(json!({"type":"string","maximumBytes":40}), json!(""))?;
    args(&mut graph)?.value = Some(ValueBinding::RunResult {
        node_id: GraphNodeId::try_new("source")?,
        pointer: "/text".into(),
    });
    assert!(compiles(&graph, &compiler)?);
    let source = GraphNodeId::try_new("source")?;
    let results = BTreeMap::from([(source, json!({"text":"voice"}))]);
    assert_eq!(
        args(&mut graph)?.assemble(&Value::Null, &BTreeMap::new(), &results),
        Some(json!("voice"))
    );
    args(&mut graph)?.value = Some(ValueBinding::Trigger {
        pointer: "/text".into(),
    });
    assert!(!compiles(&graph, &compiler)?);
    args(&mut graph)?.value = Some(ValueBinding::RunResult {
        node_id: GraphNodeId::try_new("action")?,
        pointer: String::new(),
    });
    assert!(!compiles(&graph, &compiler)?);
    Ok(())
}

#[test]
fn whole_values_require_matching_units_and_narrow_enough_numeric_ranges()
-> Result<(), Box<dyn Error>> {
    let (mut graph, compiler) = fixture(
        json!({"type":"number","minimum":-10,"maximum":10}),
        json!(0.4),
    )?;
    let mut schemas = compiler.catalog().schemas().to_vec();
    let mut endpoints = compiler.catalog().endpoints().to_vec();
    for s in &mut schemas {
        if ["org.deckweave.schema.event", "org.deckweave.schema.task"]
            .contains(&s.schema.id.as_str())
        {
            s.value_kind = SchemaValueKind::Structured(PayloadSchema::Number {
                minimum: -10.0,
                maximum: 10.0,
            });
            s.unit = Some("scalar".into());
            s.range = Some(NumericRange {
                minimum: 0.0,
                maximum: 1.0,
                center: None,
                precision: None,
            });
        }
    }
    for e in &mut endpoints {
        let s = schemas
            .iter()
            .find(|s| s.schema == e.port.schema)
            .ok_or("schema")?;
        e.port.unit = s.unit.clone();
        e.port.range = s.range.clone();
    }
    for node in &mut graph.graph.nodes {
        for port in node.inputs.iter_mut().chain(node.outputs.iter_mut()) {
            let s = schemas
                .iter()
                .find(|s| s.schema == port.schema)
                .ok_or("schema")?;
            port.unit = s.unit.clone();
            port.range = s.range.clone();
        }
    }
    let build = |schemas| -> Result<GraphPlanCompiler, Box<dyn Error>> {
        Ok(GraphPlanCompiler::new(
            DescriptorCatalog::try_new(schemas, endpoints.clone())?,
            GraphLimits::default(),
        )?)
    };
    let plan = build(schemas.clone())?.compile_document(
        &serde_json::to_value(&graph)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    for contract in &plan
        .executable_graph()
        .ok_or("executable")?
        .schema_contracts
    {
        if ["org.deckweave.schema.event", "org.deckweave.schema.task"]
            .contains(&contract.schema.id.as_str())
        {
            for value in [0.0, 0.5, 1.0] {
                assert!(contract.value_kind.accepts(&json!(value)));
            }
            for value in [-1.0, 2.0] {
                assert!(!contract.value_kind.accepts(&json!(value)));
            }
        }
    }
    args(&mut graph)?.value = Some(ValueBinding::Constant { value: json!(2) });
    assert!(!compiles(&graph, &build(schemas.clone())?)?);
    args(&mut graph)?.value = Some(ValueBinding::RunResult {
        node_id: GraphNodeId::try_new("source")?,
        pointer: String::new(),
    });
    assert!(compiles(&graph, &build(schemas.clone())?)?);
    // Keep the endpoint metadata consistent while changing only target semantics.
    let mut changed = schemas;
    changed
        .iter_mut()
        .find(|s| s.schema.id.as_str() == "org.deckweave.schema.task")
        .ok_or("schema")?
        .unit = Some("percent".into());
    endpoints
        .iter_mut()
        .find(|e| e.operation.is_some())
        .ok_or("operation")?
        .port
        .unit = Some("percent".into());
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(changed, endpoints)?,
        GraphLimits::default(),
    )?;
    assert!(!compiles(&graph, &compiler)?);
    Ok(())
}
