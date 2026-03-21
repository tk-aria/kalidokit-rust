//! drawio XML import/export for ImNodes node editor.
//!
//! Supports a subset of the mxGraphModel format: rectangle vertices + edges.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::io::Cursor;

/// A node parsed from a drawio diagram.
#[derive(Debug, Clone)]
pub struct DrawioNode {
    /// Cell ID in the drawio XML.
    pub cell_id: String,
    /// Display label.
    pub label: String,
    /// Position X in drawio coordinates.
    pub x: f32,
    /// Position Y in drawio coordinates.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// An edge parsed from a drawio diagram.
#[derive(Debug, Clone)]
pub struct DrawioEdge {
    /// Cell ID in the drawio XML.
    pub cell_id: String,
    /// Label (may be empty).
    pub label: String,
    /// Source cell ID.
    pub source: String,
    /// Target cell ID.
    pub target: String,
}

/// A full drawio diagram (nodes + edges).
#[derive(Debug, Clone, Default)]
pub struct DrawioDiagram {
    pub nodes: Vec<DrawioNode>,
    pub edges: Vec<DrawioEdge>,
}

/// ImNodes representation for rendering.
#[derive(Debug, Clone)]
pub struct ImNodesGraph {
    /// (node_id, label, x, y)
    pub nodes: Vec<(i32, String, f32, f32)>,
    /// (link_id, from_node_id_output_attr, to_node_id_input_attr)
    pub links: Vec<(i32, i32, i32)>,
    /// Mapping from drawio cell_id to ImNodes node_id.
    pub cell_to_node: HashMap<String, i32>,
}

/// Parse a drawio XML string into a DrawioDiagram.
pub fn parse_drawio(xml: &str) -> Result<DrawioDiagram, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut diagram = DrawioDiagram::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "mxCell" || tag == "object" {
                    parse_cell_attrs(e, None, &mut diagram);
                }
            }
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "mxCell" || tag == "object" {
                    // Read children to find mxGeometry
                    let geo = read_geometry(&mut reader, &tag);
                    parse_cell_attrs(e, geo.as_ref(), &mut diagram);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(diagram)
}

/// Read geometry info from inner elements of a <mxCell> or <object> start tag.
fn read_geometry(reader: &mut Reader<&[u8]>, parent_tag: &str) -> Option<(f32, f32, f32, f32)> {
    let mut result = None;
    let mut inner_buf = Vec::new();
    loop {
        match reader.read_event_into(&mut inner_buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "mxGeometry" {
                    let attrs = extract_attrs(e);
                    let x = attrs.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    let y = attrs.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    let w = attrs.get("width").and_then(|v| v.parse().ok()).unwrap_or(120.0);
                    let h = attrs.get("height").and_then(|v| v.parse().ok()).unwrap_or(60.0);
                    result = Some((x, y, w, h));
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == parent_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        inner_buf.clear();
    }
    result
}

fn parse_cell_attrs(
    e: &BytesStart,
    geo: Option<&(f32, f32, f32, f32)>,
    diagram: &mut DrawioDiagram,
) {
    let attrs = extract_attrs(e);
    let id = attrs.get("id").cloned().unwrap_or_default();

    // Skip structural cells (id=0, id=1)
    if id == "0" || id == "1" {
        return;
    }

    let label = attrs
        .get("value")
        .or_else(|| attrs.get("label"))
        .cloned()
        .unwrap_or_default();
    let is_vertex = attrs.get("vertex").map(|v| v == "1").unwrap_or(false);
    let is_edge = attrs.get("edge").map(|v| v == "1").unwrap_or(false);
    let source = attrs.get("source").cloned().unwrap_or_default();
    let target = attrs.get("target").cloned().unwrap_or_default();

    let (x, y, w, h) = geo.copied().unwrap_or((0.0, 0.0, 120.0, 60.0));

    if is_edge && !source.is_empty() && !target.is_empty() {
        diagram.edges.push(DrawioEdge {
            cell_id: id,
            label,
            source,
            target,
        });
    } else if is_vertex {
        diagram.nodes.push(DrawioNode {
            cell_id: id,
            label,
            x,
            y,
            width: w,
            height: h,
        });
    }
}

fn extract_attrs(e: &BytesStart) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = String::from_utf8_lossy(&attr.value).to_string();
        map.insert(key, val);
    }
    map
}

/// Convert a DrawioDiagram to an ImNodes graph.
///
/// Node IDs start from `base_node_id`, attribute IDs are derived as:
/// - output attr = node_id * 100 + 1
/// - input attr  = node_id * 100 + 2
pub fn diagram_to_imnodes(diagram: &DrawioDiagram, base_node_id: i32) -> ImNodesGraph {
    let mut cell_to_node: HashMap<String, i32> = HashMap::new();
    let mut nodes = Vec::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        let node_id = base_node_id + i as i32;
        cell_to_node.insert(node.cell_id.clone(), node_id);
        nodes.push((node_id, node.label.clone(), node.x, node.y));
    }

    let mut links = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        let link_id = base_node_id + 10000 + i as i32;
        if let (Some(&src_node), Some(&dst_node)) =
            (cell_to_node.get(&edge.source), cell_to_node.get(&edge.target))
        {
            let out_attr = src_node * 100 + 1;
            let in_attr = dst_node * 100 + 2;
            links.push((link_id, out_attr, in_attr));
        }
    }

    ImNodesGraph {
        nodes,
        links,
        cell_to_node,
    }
}

/// Export an ImNodes graph back to drawio XML.
pub fn imnodes_to_drawio(graph: &ImNodesGraph) -> String {
    use std::io;
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // <mxGraphModel>
    writer
        .create_element("mxGraphModel")
        .write_inner_content(|w: &mut Writer<Cursor<Vec<u8>>>| -> io::Result<()> {
            w.create_element("root")
                .write_inner_content(|w: &mut Writer<Cursor<Vec<u8>>>| -> io::Result<()> {
                    // Structural cells
                    w.create_element("mxCell")
                        .with_attribute(("id", "0"))
                        .write_empty()?;
                    w.create_element("mxCell")
                        .with_attribute(("id", "1"))
                        .with_attribute(("parent", "0"))
                        .write_empty()?;

                    // Nodes
                    for (node_id, label, x, y) in &graph.nodes {
                        let cell_id = format!("node_{}", node_id);
                        let x_str = format!("{}", x);
                        let y_str = format!("{}", y);
                        w.create_element("mxCell")
                            .with_attribute(("id", cell_id.as_str()))
                            .with_attribute(("value", label.as_str()))
                            .with_attribute(("style", "rounded=1;whiteSpace=wrap;html=1;"))
                            .with_attribute(("vertex", "1"))
                            .with_attribute(("parent", "1"))
                            .write_inner_content(|w: &mut Writer<Cursor<Vec<u8>>>| -> io::Result<()> {
                                w.create_element("mxGeometry")
                                    .with_attribute(("x", x_str.as_str()))
                                    .with_attribute(("y", y_str.as_str()))
                                    .with_attribute(("width", "120"))
                                    .with_attribute(("height", "60"))
                                    .with_attribute(("as", "geometry"))
                                    .write_empty()?;
                                Ok(())
                            })?;
                    }

                    // Edges
                    for (i, (_, out_attr, in_attr)) in graph.links.iter().enumerate() {
                        let src_node_id = out_attr / 100;
                        let dst_node_id = in_attr / 100;
                        let src_cell = format!("node_{}", src_node_id);
                        let dst_cell = format!("node_{}", dst_node_id);
                        let edge_id = format!("edge_{}", i);
                        w.create_element("mxCell")
                            .with_attribute(("id", edge_id.as_str()))
                            .with_attribute(("value", ""))
                            .with_attribute(("style", "endArrow=classic;html=1;rounded=0;"))
                            .with_attribute(("edge", "1"))
                            .with_attribute(("parent", "1"))
                            .with_attribute(("source", src_cell.as_str()))
                            .with_attribute(("target", dst_cell.as_str()))
                            .write_inner_content(|w: &mut Writer<Cursor<Vec<u8>>>| -> io::Result<()> {
                                w.create_element("mxGeometry")
                                    .with_attribute(("relative", "1"))
                                    .with_attribute(("as", "geometry"))
                                    .write_empty()?;
                                Ok(())
                            })?;
                    }

                    Ok(())
                })?;
            Ok(())
        })
        .unwrap();

    String::from_utf8(writer.into_inner().into_inner()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DRAWIO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mxGraphModel>
  <root>
    <mxCell id="0"/>
    <mxCell id="1" parent="0"/>
    <mxCell id="2" value="Camera" style="rounded=1;" vertex="1" parent="1">
      <mxGeometry x="100" y="50" width="120" height="60" as="geometry"/>
    </mxCell>
    <mxCell id="3" value="Tracker" style="rounded=1;" vertex="1" parent="1">
      <mxGeometry x="300" y="50" width="120" height="60" as="geometry"/>
    </mxCell>
    <mxCell id="4" value="" edge="1" source="2" target="3" parent="1">
      <mxGeometry relative="1" as="geometry"/>
    </mxCell>
  </root>
</mxGraphModel>"#;

    #[test]
    fn parse_and_convert() {
        let diagram = parse_drawio(SAMPLE_DRAWIO).unwrap();
        assert_eq!(diagram.nodes.len(), 2);
        assert_eq!(diagram.edges.len(), 1);
        assert_eq!(diagram.nodes[0].label, "Camera");
        assert_eq!(diagram.nodes[1].label, "Tracker");

        let graph = diagram_to_imnodes(&diagram, 100);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.links.len(), 1);
    }

    #[test]
    fn roundtrip_export() {
        let diagram = parse_drawio(SAMPLE_DRAWIO).unwrap();
        let graph = diagram_to_imnodes(&diagram, 100);
        let xml = imnodes_to_drawio(&graph);
        assert!(xml.contains("Camera"));
        assert!(xml.contains("Tracker"));
        assert!(xml.contains("edge"));

        // Re-parse the exported XML
        let diagram2 = parse_drawio(&xml).unwrap();
        assert_eq!(diagram2.nodes.len(), 2);
        assert_eq!(diagram2.edges.len(), 1);
    }
}
