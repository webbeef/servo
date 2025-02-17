/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS table formatting contexts.

use std::fmt;

use app_units::Au;
use base::print_tree::PrintTree;
use euclid::default::{Point2D, Rect, SideOffsets2D, Size2D};
use log::{debug, trace};
use script_layout_interface::wrapper_traits::ThreadSafeLayoutNode;
use serde::Serialize;
use style::logical_geometry::{LogicalMargin, LogicalRect, LogicalSize, WritingMode};
use style::properties::ComputedValues;
use style::values::computed::Color;
use style::values::generics::box_::{VerticalAlign, VerticalAlignKeyword};
use style::values::specified::BorderStyle;

use crate::block::{BlockFlow, ISizeAndMarginsComputer, MarginsMayCollapseFlag};
use crate::context::LayoutContext;
use crate::display_list::{
    DisplayListBuildState, StackingContextCollectionFlags, StackingContextCollectionState,
};
use crate::flow::{Flow, FlowClass, FlowFlags, GetBaseFlow, OpaqueFlow};
use crate::fragment::{Fragment, FragmentBorderBoxIterator, Overflow};
use crate::table::InternalTable;
use crate::table_row::{CollapsedBorder, CollapsedBorderFrom};

#[allow(unsafe_code)]
unsafe impl crate::flow::HasBaseFlow for TableCellFlow {}

/// A table formatting context.
#[derive(Serialize)]
#[repr(C)]
pub struct TableCellFlow {
    /// Data common to all block flows.
    pub block_flow: BlockFlow,

    /// Border collapse information for the cell.
    pub collapsed_borders: CollapsedBordersForCell,

    /// The column span of this cell.
    pub column_span: u32,

    /// The rows spanned by this cell.
    pub row_span: u32,

    /// Whether this cell is visible. If false, the value of `empty-cells` means that we must not
    /// display this cell.
    pub visible: bool,
}

impl TableCellFlow {
    pub fn from_fragment(fragment: Fragment) -> TableCellFlow {
        TableCellFlow {
            block_flow: BlockFlow::from_fragment(fragment),
            collapsed_borders: CollapsedBordersForCell::new(),
            column_span: 1,
            row_span: 1,
            visible: true,
        }
    }

    pub fn from_node_fragment_and_visibility_flag<'dom>(
        node: &impl ThreadSafeLayoutNode<'dom>,
        fragment: Fragment,
        visible: bool,
    ) -> TableCellFlow {
        TableCellFlow {
            block_flow: BlockFlow::from_fragment(fragment),
            collapsed_borders: CollapsedBordersForCell::new(),
            column_span: node.get_colspan().unwrap_or(1),
            row_span: node.get_rowspan().unwrap_or(1),
            visible,
        }
    }

    pub fn fragment(&mut self) -> &Fragment {
        &self.block_flow.fragment
    }

    pub fn mut_fragment(&mut self) -> &mut Fragment {
        &mut self.block_flow.fragment
    }

    /// Assign block-size for table-cell flow.
    ///
    /// inline(always) because this is only ever called by in-order or non-in-order top-level
    /// methods.
    #[inline(always)]
    fn assign_block_size_table_cell_base(&mut self, layout_context: &LayoutContext) {
        let remaining = self.block_flow.assign_block_size_block_base(
            layout_context,
            None,
            MarginsMayCollapseFlag::MarginsMayNotCollapse,
        );
        debug_assert!(remaining.is_none());
    }

    /// Position this cell's children according to vertical-align.
    pub fn valign_children(&mut self) {
        // Note to the reader: this code has been tested with negative margins.
        // We end up with a "end" that's before the "start," but the math still works out.
        let mut extents = None;
        for kid in self.base().children.iter() {
            let kid_base = kid.base();
            if kid_base.flags.contains(FlowFlags::IS_ABSOLUTELY_POSITIONED) {
                continue;
            }
            let start = kid_base.position.start.b -
                kid_base
                    .collapsible_margins
                    .block_start_margin_for_noncollapsible_context();
            let end = kid_base.position.start.b +
                kid_base.position.size.block +
                kid_base
                    .collapsible_margins
                    .block_end_margin_for_noncollapsible_context();
            match extents {
                Some((ref mut first_start, ref mut last_end)) => {
                    if start < *first_start {
                        *first_start = start
                    }
                    if end > *last_end {
                        *last_end = end
                    }
                },
                None => extents = Some((start, end)),
            }
        }
        let (first_start, last_end) = match extents {
            Some(extents) => extents,
            None => return,
        };

        let kids_size = last_end - first_start;
        let self_size = self.base().position.size.block -
            self.block_flow.fragment.border_padding.block_start_end();
        let kids_self_gap = self_size - kids_size;

        // This offset should also account for VerticalAlign::baseline.
        // Need max cell ascent from the first row of this cell.
        let offset = match self.block_flow.fragment.style().get_box().vertical_align {
            VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => kids_self_gap / 2,
            VerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => kids_self_gap,
            _ => Au(0),
        };
        if offset == Au(0) {
            return;
        }

        for kid in self.mut_base().children.iter_mut() {
            let kid_base = kid.mut_base();
            if !kid_base.flags.contains(FlowFlags::IS_ABSOLUTELY_POSITIONED) {
                kid_base.position.start.b += offset
            }
        }
    }

    // Total block size of child
    //
    // Call after block size calculation
    pub fn total_block_size(&mut self) -> Au {
        // TODO: Percentage block-size
        let specified = self
            .fragment()
            .style()
            .content_block_size()
            .to_used_value(Au(0))
            .unwrap_or(Au(0));
        specified + self.fragment().border_padding.block_start_end()
    }
}

impl Flow for TableCellFlow {
    fn class(&self) -> FlowClass {
        FlowClass::TableCell
    }

    fn as_mut_table_cell(&mut self) -> &mut TableCellFlow {
        self
    }

    fn as_table_cell(&self) -> &TableCellFlow {
        self
    }

    fn as_mut_block(&mut self) -> &mut BlockFlow {
        &mut self.block_flow
    }

    fn as_block(&self) -> &BlockFlow {
        &self.block_flow
    }

    /// Minimum/preferred inline-sizes set by this function are used in automatic table layout
    /// calculation.
    fn bubble_inline_sizes(&mut self) {
        self.block_flow.bubble_inline_sizes_for_block(true);
        let specified_inline_size = self
            .block_flow
            .fragment
            .style()
            .content_inline_size()
            .to_used_value(Au(0))
            .unwrap_or(Au(0));

        if self
            .block_flow
            .base
            .intrinsic_inline_sizes
            .minimum_inline_size <
            specified_inline_size
        {
            self.block_flow
                .base
                .intrinsic_inline_sizes
                .minimum_inline_size = specified_inline_size
        }
        if self
            .block_flow
            .base
            .intrinsic_inline_sizes
            .preferred_inline_size <
            self.block_flow
                .base
                .intrinsic_inline_sizes
                .minimum_inline_size
        {
            self.block_flow
                .base
                .intrinsic_inline_sizes
                .preferred_inline_size = self
                .block_flow
                .base
                .intrinsic_inline_sizes
                .minimum_inline_size;
        }
    }

    /// Recursively (top-down) determines the actual inline-size of child contexts and fragments.
    /// When called on this context, the context has had its inline-size set by the parent table
    /// row.
    fn assign_inline_sizes(&mut self, layout_context: &LayoutContext) {
        debug!(
            "assign_inline_sizes({}): assigning inline_size for flow",
            "table_cell"
        );
        trace!("TableCellFlow before assigning: {:?}", &self);

        let shared_context = layout_context.shared_context();
        // The position was set to the column inline-size by the parent flow, table row flow.
        let containing_block_inline_size = self.block_flow.base.block_container_inline_size;

        let inline_size_computer = InternalTable;
        inline_size_computer.compute_used_inline_size(
            &mut self.block_flow,
            shared_context,
            containing_block_inline_size,
        );

        let inline_start_content_edge = self.block_flow.fragment.border_box.start.i +
            self.block_flow.fragment.border_padding.inline_start;
        let inline_end_content_edge = self.block_flow.base.block_container_inline_size -
            self.block_flow.fragment.border_padding.inline_start_end() -
            self.block_flow.fragment.border_box.size.inline;
        let padding_and_borders = self.block_flow.fragment.border_padding.inline_start_end();
        let content_inline_size =
            self.block_flow.fragment.border_box.size.inline - padding_and_borders;

        self.block_flow.propagate_assigned_inline_size_to_children(
            shared_context,
            inline_start_content_edge,
            inline_end_content_edge,
            content_inline_size,
            |_, _, _, _, _, _| {},
        );

        trace!("TableCellFlow after assigning: {:?}", &self);
    }

    fn assign_block_size(&mut self, layout_context: &LayoutContext) {
        debug!("assign_block_size: assigning block_size for table_cell");
        trace!("TableCellFlow before assigning: {:?}", &self);

        self.assign_block_size_table_cell_base(layout_context);

        trace!("TableCellFlow after assigning: {:?}", &self);
    }

    fn compute_stacking_relative_position(&mut self, layout_context: &LayoutContext) {
        self.block_flow
            .compute_stacking_relative_position(layout_context)
    }

    fn update_late_computed_inline_position_if_necessary(&mut self, inline_position: Au) {
        self.block_flow
            .update_late_computed_inline_position_if_necessary(inline_position)
    }

    fn update_late_computed_block_position_if_necessary(&mut self, block_position: Au) {
        self.block_flow
            .update_late_computed_block_position_if_necessary(block_position)
    }

    fn build_display_list(&mut self, _: &mut DisplayListBuildState) {
        use style::servo::restyle_damage::ServoRestyleDamage;
        // This is handled by TableCellStyleInfo::build_display_list()
        // when the containing table builds its display list

        // we skip setting the damage in TableCellStyleInfo::build_display_list()
        // because we only have immutable access
        self.block_flow
            .fragment
            .restyle_damage
            .remove(ServoRestyleDamage::REPAINT);
    }

    fn collect_stacking_contexts(&mut self, state: &mut StackingContextCollectionState) {
        self.block_flow
            .collect_stacking_contexts_for_block(state, StackingContextCollectionFlags::empty());
    }

    fn repair_style(&mut self, new_style: &crate::ServoArc<ComputedValues>) {
        self.block_flow.repair_style(new_style)
    }

    fn compute_overflow(&self) -> Overflow {
        self.block_flow.compute_overflow()
    }

    fn contains_roots_of_absolute_flow_tree(&self) -> bool {
        self.block_flow.contains_roots_of_absolute_flow_tree()
    }

    fn is_absolute_containing_block(&self) -> bool {
        self.block_flow.is_absolute_containing_block()
    }

    fn generated_containing_block_size(&self, flow: OpaqueFlow) -> LogicalSize<Au> {
        self.block_flow.generated_containing_block_size(flow)
    }

    fn iterate_through_fragment_border_boxes(
        &self,
        iterator: &mut dyn FragmentBorderBoxIterator,
        level: i32,
        stacking_context_position: &Point2D<Au>,
    ) {
        self.block_flow.iterate_through_fragment_border_boxes(
            iterator,
            level,
            stacking_context_position,
        )
    }

    fn mutate_fragments(&mut self, mutator: &mut dyn FnMut(&mut Fragment)) {
        self.block_flow.mutate_fragments(mutator)
    }

    fn print_extra_flow_children(&self, print_tree: &mut PrintTree) {
        self.block_flow.print_extra_flow_children(print_tree);
    }
}

impl fmt::Debug for TableCellFlow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TableCellFlow: {:?}", self.block_flow)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CollapsedBordersForCell {
    pub inline_start_border: CollapsedBorder,
    pub inline_end_border: CollapsedBorder,
    pub block_start_border: CollapsedBorder,
    pub block_end_border: CollapsedBorder,
    pub inline_start_width: Au,
    pub inline_end_width: Au,
    pub block_start_width: Au,
    pub block_end_width: Au,
}

impl CollapsedBordersForCell {
    fn new() -> CollapsedBordersForCell {
        CollapsedBordersForCell {
            inline_start_border: CollapsedBorder::new(),
            inline_end_border: CollapsedBorder::new(),
            block_start_border: CollapsedBorder::new(),
            block_end_border: CollapsedBorder::new(),
            inline_start_width: Au(0),
            inline_end_width: Au(0),
            block_start_width: Au(0),
            block_end_width: Au(0),
        }
    }

    fn should_paint_inline_start_border(&self) -> bool {
        self.inline_start_border.provenance != CollapsedBorderFrom::PreviousTableCell
    }

    fn should_paint_inline_end_border(&self) -> bool {
        self.inline_end_border.provenance != CollapsedBorderFrom::NextTableCell
    }

    fn should_paint_block_start_border(&self) -> bool {
        self.block_start_border.provenance != CollapsedBorderFrom::PreviousTableCell
    }

    fn should_paint_block_end_border(&self) -> bool {
        self.block_end_border.provenance != CollapsedBorderFrom::NextTableCell
    }

    pub fn adjust_border_widths_for_painting(&self, border_widths: &mut LogicalMargin<Au>) {
        border_widths.inline_start = if !self.should_paint_inline_start_border() {
            Au(0)
        } else {
            self.inline_start_border.width
        };
        border_widths.inline_end = if !self.should_paint_inline_end_border() {
            Au(0)
        } else {
            self.inline_end_border.width
        };
        border_widths.block_start = if !self.should_paint_block_start_border() {
            Au(0)
        } else {
            self.block_start_border.width
        };
        border_widths.block_end = if !self.should_paint_block_end_border() {
            Au(0)
        } else {
            self.block_end_border.width
        }
    }

    pub fn adjust_border_bounds_for_painting(
        &self,
        border_bounds: &mut Rect<Au>,
        writing_mode: WritingMode,
    ) {
        let inline_start_divisor = if self.should_paint_inline_start_border() {
            2
        } else {
            -2
        };
        let inline_start_offset =
            self.inline_start_width / 2 + self.inline_start_border.width / inline_start_divisor;
        let inline_end_divisor = if self.should_paint_inline_end_border() {
            2
        } else {
            -2
        };
        let inline_end_offset =
            self.inline_end_width / 2 + self.inline_end_border.width / inline_end_divisor;
        let block_start_divisor = if self.should_paint_block_start_border() {
            2
        } else {
            -2
        };
        let block_start_offset =
            self.block_start_width / 2 + self.block_start_border.width / block_start_divisor;
        let block_end_divisor = if self.should_paint_block_end_border() {
            2
        } else {
            -2
        };
        let block_end_offset =
            self.block_end_width / 2 + self.block_end_border.width / block_end_divisor;

        // FIXME(pcwalton): Get the real container size.
        let mut logical_bounds =
            LogicalRect::from_physical(writing_mode, *border_bounds, Size2D::new(Au(0), Au(0)));
        logical_bounds.start.i -= inline_start_offset;
        logical_bounds.start.b -= block_start_offset;
        logical_bounds.size.inline =
            logical_bounds.size.inline + inline_start_offset + inline_end_offset;
        logical_bounds.size.block =
            logical_bounds.size.block + block_start_offset + block_end_offset;
        *border_bounds = logical_bounds.to_physical(writing_mode, Size2D::new(Au(0), Au(0)))
    }

    pub fn adjust_border_colors_and_styles_for_painting(
        &self,
        border_colors: &mut SideOffsets2D<Color>,
        border_styles: &mut SideOffsets2D<BorderStyle>,
        writing_mode: WritingMode,
    ) {
        let logical_border_colors = LogicalMargin::new(
            writing_mode,
            self.block_start_border.color.clone(),
            self.inline_end_border.color.clone(),
            self.block_end_border.color.clone(),
            self.inline_start_border.color.clone(),
        );
        *border_colors = logical_border_colors.to_physical(writing_mode);

        let logical_border_styles = LogicalMargin::new(
            writing_mode,
            self.block_start_border.style,
            self.inline_end_border.style,
            self.block_end_border.style,
            self.inline_start_border.style,
        );
        *border_styles = logical_border_styles.to_physical(writing_mode);
    }
}
