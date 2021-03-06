use dom::node::Node;
use layout::block::BlockFlowData;
use layout::box::RenderBox;
use layout::context::LayoutContext;
use layout::debug::BoxedDebugMethods;
use layout::display_list_builder::DisplayListBuilder;
use layout::inline::{InlineFlowData, NodeRange};
use layout::root::RootFlowData;
use util::tree;

use core::dvec::DVec;
use geom::rect::Rect;
use geom::point::Point2D;
use gfx::display_list::DisplayList;
use gfx::geometry::Au;

/** Servo's experimental layout system builds a tree of FlowContexts
and RenderBoxes, and figures out positions and display attributes of
tree nodes. Positions are computed in several tree traversals driven
by fundamental data dependencies of inline and block layout.

Flows are interior nodes in the layout tree, and correspond closely to
flow contexts in the CSS specification. Flows are responsible for
positioning their child flow contexts and render boxes. Flows have
purpose-specific fields, such as auxilliary line box structs,
out-of-flow child lists, and so on.

Currently, the important types of flows are:

 * BlockFlow: a flow that establishes a block context. It has several
   child flows, each of which are positioned according to block
   formatting context rules (as if child flows CSS block boxes). Block
   flows also contain a single GenericBox to represent their rendered
   borders, padding, etc. (In the future, this render box may be
   folded into BlockFlow to save space.)

 * InlineFlow: a flow that establishes an inline context. It has a
   flat list of child boxes/flows that are subject to inline layout
   and line breaking, and structs to represent line breaks and mapping
   to CSS boxes, for the purpose of handling `getClientRects()`.

*/

/* The type of the formatting context, and data specific to each
context, such as linebox structures or float lists */ 
pub enum FlowContext {
    AbsoluteFlow(FlowData), 
    BlockFlow(FlowData, BlockFlowData),
    FloatFlow(FlowData),
    InlineBlockFlow(FlowData),
    InlineFlow(FlowData, InlineFlowData),
    RootFlow(FlowData, RootFlowData),
    TableFlow(FlowData)
}

enum FlowContextType {
    Flow_Absolute, 
    Flow_Block,
    Flow_Float,
    Flow_InlineBlock,
    Flow_Inline,
    Flow_Root,
    Flow_Table
}

/* A particular kind of layout context. It manages the positioning of
   render boxes within the context.  */
struct FlowData {
    mut node: Option<Node>,
    /* reference to parent, children flow contexts */
    tree: tree::Tree<@FlowContext>,
    /* TODO (Issue #87): debug only */
    mut id: int,

    /* layout computations */
    // TODO: min/pref and position are used during disjoint phases of
    // layout; maybe combine into a single enum to save space.
    mut min_width: Au,
    mut pref_width: Au,
    mut position: Rect<Au>,
}

fn FlowData(id: int) -> FlowData {
    FlowData {
        node: None,
        tree: tree::empty(),
        id: id,

        min_width: Au(0),
        pref_width: Au(0),
        position: Au::zero_rect()
    }
}

impl FlowContext  {
    pure fn d(&self) -> &self/FlowData {
        match *self {
            AbsoluteFlow(ref d)    => d,
            BlockFlow(ref d, _)    => d,
            FloatFlow(ref d)       => d,
            InlineBlockFlow(ref d) => d,
            InlineFlow(ref d, _)   => d,
            RootFlow(ref d, _)     => d,
            TableFlow(ref d)       => d
        }
    }

    pure fn inline(&self) -> &self/InlineFlowData {
        match *self {
            InlineFlow(_, ref i) => i,
            _ => fail fmt!("Tried to access inline data of non-inline: f%d", self.d().id)
        }
    }

    pure fn block(&self) -> &self/BlockFlowData {
        match *self {
            BlockFlow(_, ref b) => b,
            _ => fail fmt!("Tried to access block data of non-block: f%d", self.d().id)
        }
    }

    pure fn root(&self) -> &self/RootFlowData {
        match *self {
            RootFlow(_, ref r) => r,
            _ => fail fmt!("Tried to access root data of non-root: f%d", self.d().id)
        }
    }

    fn bubble_widths(@self, ctx: &LayoutContext) {
        match self {
            @BlockFlow(*)  => self.bubble_widths_block(ctx),
            @InlineFlow(*) => self.bubble_widths_inline(ctx),
            @RootFlow(*)   => self.bubble_widths_root(ctx),
            _ => fail fmt!("Tried to bubble_widths of flow: f%d", self.d().id)
        }
    }

    fn assign_widths(@self, ctx: &LayoutContext) {
        match self {
            @BlockFlow(*)  => self.assign_widths_block(ctx),
            @InlineFlow(*) => self.assign_widths_inline(ctx),
            @RootFlow(*)   => self.assign_widths_root(ctx),
            _ => fail fmt!("Tried to assign_widths of flow: f%d", self.d().id)
        }
    }

    fn assign_height(@self, ctx: &LayoutContext) {
        match self {
            @BlockFlow(*)  => self.assign_height_block(ctx),
            @InlineFlow(*) => self.assign_height_inline(ctx),
            @RootFlow(*)   => self.assign_height_root(ctx),
            _ => fail fmt!("Tried to assign_height of flow: f%d", self.d().id)
        }
    }

    fn build_display_list_recurse(@self, builder: &DisplayListBuilder, dirty: &Rect<Au>,
                                  offset: &Point2D<Au>, list: &mut DisplayList) {
        debug!("FlowContext::build_display_list at %?: %s", self.d().position, self.debug_str());

        match self {
            @RootFlow(*) => self.build_display_list_root(builder, dirty, offset, list),
            @BlockFlow(*) => self.build_display_list_block(builder, dirty, offset, list),
            @InlineFlow(*) => self.build_display_list_inline(builder, dirty, offset, list),
            _ => fail fmt!("Tried to build_display_list_recurse of flow: %?", self)
        }
    }

    // Actual methods that do not require much flow-specific logic
    pure fn foldl_all_boxes<B: Copy>(seed: B, 
                                     cb: pure fn&(a: B,@RenderBox) -> B) -> B {
        match self {
            RootFlow(*)   => option::map_default(&self.root().box, seed, |box| { cb(seed, *box) }),
            BlockFlow(*)  => option::map_default(&self.block().box, seed, |box| { cb(seed, *box) }),
            InlineFlow(*) => do self.inline().boxes.foldl(seed) |acc, box| { cb(*acc, *box) },
            _ => fail fmt!("Don't know how to iterate node's RenderBoxes for %?", self)
        }
    }

    pure fn foldl_boxes_for_node<B: Copy>(node: Node, seed: B, 
                                          cb: pure fn&(a: B,@RenderBox) -> B) -> B {
        do self.foldl_all_boxes(seed) |acc, box| {
            if box.d().node == node { cb(acc, box) }
            else { acc }
        }
    }

    pure fn iter_all_boxes<T>(cb: pure fn&(@RenderBox) -> T) {
        match self {
            RootFlow(*)   => do self.root().box.iter |box| { cb(*box); },
            BlockFlow(*)  => do self.block().box.iter |box| { cb(*box); },
            InlineFlow(*) => for self.inline().boxes.each |box| { cb(*box); },
            _ => fail fmt!("Don't know how to iterate node's RenderBoxes for %?", self)
        }
    }

    pure fn iter_boxes_for_node<T>(node: Node,
                                   cb: pure fn&(@RenderBox) -> T) {
        do self.iter_all_boxes |box| {
            if box.d().node == node { cb(box); }
        }
    }
}

/* The tree holding FlowContexts */
pub enum FlowTree { FlowTree }

impl FlowTree {
    fn each_child(ctx: @FlowContext, f: fn(box: @FlowContext) -> bool) {
        tree::each_child(&self, &ctx, |box| f(*box) )
    }
}

impl FlowTree : tree::ReadMethods<@FlowContext> {
    fn with_tree_fields<R>(box: &@FlowContext, f: fn(&tree::Tree<@FlowContext>) -> R) -> R {
        f(&box.d().tree)
    }
}

impl FlowTree {
    fn add_child(parent: @FlowContext, child: @FlowContext) {
        tree::add_child(&self, parent, child)
    }
}

impl FlowTree : tree::WriteMethods<@FlowContext> {

    pure fn eq(a: &@FlowContext, b: &@FlowContext) -> bool { core::managed::ptr_eq(*a, *b) }

    fn with_tree_fields<R>(box: &@FlowContext, f: fn(&tree::Tree<@FlowContext>) -> R) -> R {
        f(&box.d().tree)
    }
}


impl FlowContext : BoxedDebugMethods {
    pure fn dump(@self) {
        self.dump_indent(0u);
    }

    /** Dumps the flow tree, for debugging, with indentation. */
    pure fn dump_indent(@self, indent: uint) {
        let mut s = ~"|";
        for uint::range(0u, indent) |_i| {
            s += ~"---- ";
        }

        s += self.debug_str();
        debug!("%s", s);

        // FIXME: this should have a pure/const version?
        unsafe {
            for FlowTree.each_child(self) |child| {
                child.dump_indent(indent + 1u) 
            }
        }
    }
    
    pure fn debug_str(@self) -> ~str {
        let repr = match *self {
            InlineFlow(*) => {
                let mut s = self.inline().boxes.foldl(~"InlineFlow(children=", |s, box| {
                    fmt!("%s b%d", *s, box.d().id)
                });
                s += ~")";
                move s
            },
            BlockFlow(*) => {
                match self.block().box {
                    Some(box) => fmt!("BlockFlow(box=b%d)", box.d().id),
                    None => ~"BlockFlow",
                }
            },
            RootFlow(*) => {
                match self.root().box {
                    Some(box) => fmt!("RootFlo(box=b%d)", box.d().id),
                    None => ~"RootFlow",
                }
            },
            _ => ~"(Unknown flow)"
        };
            
        fmt!("f%? %?", self.d().id, repr)
    }
}
