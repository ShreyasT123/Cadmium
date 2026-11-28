fn paint_node(
    wid: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    pctx: &mut PaintCtx<'_, '_>,
    offset: (f32, f32),
) {
    let Some(node) = nodes.get(wid) else {
        return;
    };

    let bounds = Rect::new(
        node.bounds.x + offset.0,
        node.bounds.y + offset.1,
        node.bounds.width,
        node.bounds.height,
    );

    let scale = pctx.scale;
    let theme = pctx.theme;
    let is_hovered = pctx.hovered == Some(wid);
    let is_focused = pctx.focused == Some(wid);

    if let Some(a11y) = pctx.a11y_records.as_deref_mut() {
        let (role, label, value, numeric_value, toggled, focusable, clickable) = match &node.payload
        {
            WidgetPayload::Text { block, .. } => (
                A11yRole::Label,
                node.tooltip.clone(),
                Some(format!("{:.0}", block.style().font_size)),
                None,
                None,
                false,
                false,
            ),
            WidgetPayload::Button { .. } => (
                A11yRole::Button,
                node.tooltip.clone(),
                None,
                None,
                None,
                true,
                true,
            ),
            WidgetPayload::Switch { signal } => (
                A11yRole::Switch,
                node.tooltip.clone().or_else(|| Some("Switch".into())),
                None,
                None,
                signal.try_get(),
                true,
                true,
            ),
            WidgetPayload::Slider { signal, .. } => (
                A11yRole::Slider,
                node.tooltip.clone().or_else(|| Some("Slider".into())),
                None,
                signal.try_get().map(|v| v as f64),
                None,
                true,
                false,
            ),
            WidgetPayload::TextInput { signal, .. } => (
                A11yRole::TextInput,
                node.tooltip.clone().or_else(|| Some("Text Input".into())),
                signal.try_get(),
                None,
                None,
                true,
                true,
            ),
            WidgetPayload::TextArea { signal, .. } => (
                A11yRole::TextInput,
                node.tooltip.clone().or_else(|| Some("Text Area".into())),
                signal.try_get(),
                None,
                None,
                true,
                true,
            ),
            _ => (A11yRole::Container, None, None, None, None, false, false),
        };
        a11y.push(A11yNodeRecord {
            id: wid,
            role,
            label,
            value,
            numeric_value,
            toggled,
            focusable,
            clickable,
            bounds: bounds.scale(scale),
            children: node.children.clone(),
        });
    }

    match &node.payload {
        WidgetPayload::Container {
            material,
            radius,
            hover_fill,
            ..
        } => {
            render_material(pctx.ctx, bounds, material.get(), radius.get(), scale);
            if is_hovered && let Some(fill) = *hover_fill {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius.get()).scale(scale),
                    fill,
                    ..Default::default()
                });
            }
            if node.layout_style.clip {
                pctx.ctx.push_clip(bounds);
            }
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
            if node.layout_style.clip {
                pctx.ctx.pop_clip();
            }
        }
        WidgetPayload::Switcher {
            signal,
            prev_idx,
            transition,
            ..
        } => {
            let active = signal.try_get().unwrap_or(0);
            let t = transition.value.clamp(0.0, 1.0);
            if t < 0.999 && prev_idx.is_some() {
                pctx.ctx.push_clip(bounds);
                if let Some(prev) = *prev_idx
                    && let Some(&prev_child) = node.children.get(prev)
                {
                    pctx.ctx.push_opacity(1.0 - t);
                    paint_node(prev_child, nodes, pctx, (offset.0 - 26.0 * t, offset.1));
                    pctx.ctx.pop_opacity();
                }
                if let Some(&active_child) = node.children.get(active) {
                    pctx.ctx.push_opacity(t);
                    paint_node(
                        active_child,
                        nodes,
                        pctx,
                        (offset.0 + 26.0 * (1.0 - t), offset.1),
                    );
                    pctx.ctx.pop_opacity();
                }
                pctx.ctx.pop_clip();
            } else if let Some(&active_child) = node.children.get(active) {
                paint_node(active_child, nodes, pctx, offset);
            }
        }
        WidgetPayload::Show { .. } | WidgetPayload::ForEach { .. } => {
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
        }
        WidgetPayload::Text { block, tone, .. } => {
            let color = tone.resolve(theme, block.style().color);
            let clipped = block.is_single_line();
            if clipped {
                pctx.ctx.push_clip(bounds);
            }
            pctx.ctx.push_text_colored(block, bounds.x, bounds.y, color);
            if clipped {
                pctx.ctx.pop_clip();
            }
        }
        WidgetPayload::Image {
            state,
            radius,
            tint,
            shadow,
            fit,
            ..
        } => {
            if let Some(shadow) = shadow {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(*radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: shadow.color,
                    shadow_offset: [shadow.offset[0] * scale, shadow.offset[1] * scale],
                    shadow_blur: shadow.blur * scale,
                    ..Default::default()
                });
            }
            let mut st = state.borrow_mut();
            if st.handle.is_none() {
                if let Some(key) = st.source_key {
                    if let Some((handle, refs)) = pctx.decoded_tex_cache.get_mut(&key) {
                        *refs += 1;
                        st.handle = Some(*handle);
                        st.image = None;
                    } else if let Some(img) = st.image.take() {
                        let handle = pctx.ctx.gpu_mut().upload_image(img.as_ref());
                        pctx.decoded_tex_cache.insert(key, (handle, 1));
                        st.handle = Some(handle);
                    }
                } else if let Some(img) = st.image.take() {
                    st.handle = Some(pctx.ctx.gpu_mut().upload_image(img.as_ref()));
                }
            }
            if let Some(handle) = st.handle {
                let uv = match fit {
                    ImageFit::Cover => {
                        cover_uv_rect(st.width, st.height, bounds.width, bounds.height)
                    }
                    ImageFit::Contain | ImageFit::Stretch => [0.0, 0.0, 1.0, 1.0],
                };
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    uv_rect: uv,
                    radii: CornerRadii::all(*radius).scale(scale),
                    tint: *tint,
                    texture: handle,
                });
            }
        }
        WidgetPayload::Icon {
            state,
            tone,
            color,
            size,
        } => {
            let mut st = state.borrow_mut();
            if st.handle.is_none()
                && let Some(img) = st.image.take()
            {
                st.handle = Some(pctx.ctx.gpu_mut().upload_image(&img));
            }
            if let Some(handle) = st.handle {
                let tint = tone.resolve(theme, *color);
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(0.0),
                    tint,
                    texture: handle,
                });
            }
            let _ = size;
        }
        WidgetPayload::ProgressBar { signal, color } => {
            let v = signal.try_get().unwrap_or(0.0).clamp(0.0, 1.0);
            let track = snap_rect_to_pixels(bounds, scale);
            pctx.ctx.push_sdf(SdfQuad {
                rect: track,
                radii: CornerRadii::all(bounds.height * 0.5).scale(scale),
                fill: theme.hairline,
                ..Default::default()
            });
            if v > 0.0 {
                let fill_rect = Rect::new(bounds.x, bounds.y, bounds.width * v, bounds.height);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(fill_rect, scale),
                    radii: CornerRadii::all(bounds.height * 0.5).scale(scale),
                    fill: color.unwrap_or(theme.accent),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::Button {
            label,
            label_empty,
            accent,
            variant,
            icon_state,
            icon_size,
            pill,
            ..
        } => {
            let r = if *pill { bounds.height * 0.5 } else { 7.0 };
            let is_primary = *variant == ButtonVariant::Primary || *accent;
            let is_glass = *variant == ButtonVariant::Glass;
            let is_ghost = *variant == ButtonVariant::Ghost;
            if is_glass {
                pctx.ctx.push_glass(GlassQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(r).scale(scale),
                    tint: if is_hovered {
                        Color::from_rgba8(255, 255, 255, 34)
                    } else {
                        Color::from_rgba8(255, 255, 255, 24)
                    },
                    saturation: 1.0,
                    frost: 0.22,
                    liquid_ripple: 0.0,
                    specular: 0.8,
                    rim_angle: -1.309,
                    sheen_progress: 0.0,
                    conic_strength: 0.0,
                    border_color: theme.hairline,
                    border_width: 1.0 * scale,
                    ..Default::default()
                });
            } else {
                let fill = if is_primary {
                    theme.accent
                } else if is_ghost {
                    if is_hovered {
                        theme.control_hover
                    } else {
                        Color::TRANSPARENT
                    }
                } else if is_hovered {
                    theme.control_hover
                } else {
                    theme.panel_bg
                };
                let border_color = if is_primary || is_ghost {
                    Color::TRANSPARENT
                } else {
                    theme.hairline
                };
                let border_width = if is_primary || is_ghost {
                    0.0
                } else {
                    1.0 * scale
                };
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(r).scale(scale),
                    fill,
                    border_color,
                    border_width,
                    shadow_color: if is_ghost {
                        Color::TRANSPARENT
                    } else {
                        theme.shadow_soft
                    },
                    shadow_offset: [0.0, 1.0 * scale],
                    shadow_blur: if is_ghost { 0.0 } else { 3.0 * scale },
                    ..Default::default()
                });
            }

            let mut st = icon_state.borrow_mut();
            if st.handle.is_none()
                && let Some(img) = st.image.take()
            {
                st.handle = Some(pctx.ctx.gpu_mut().upload_image(&img));
            }
            let has_icon = st.handle.is_some();
            let text_color = if is_primary {
                theme.text_on_accent
            } else {
                theme.text_primary
            };
            let icon_color = text_color;
            let mut text_x = bounds.x + 14.0;
            if let Some(handle) = st.handle {
                let ix = if *label_empty {
                    bounds.x + (bounds.width - *icon_size) * 0.5
                } else {
                    bounds.x + 10.0
                };
                let iy = bounds.y + (bounds.height - *icon_size) * 0.5;
                pctx.ctx.push_image(ImageQuad {
                    rect: snap_rect_to_pixels(Rect::new(ix, iy, *icon_size, *icon_size), scale),
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(0.0),
                    tint: icon_color,
                    texture: handle,
                });
                text_x = ix + *icon_size + 8.0;
            }
            if !*label_empty {
                pctx.ctx
                    .push_text_colored(label, text_x, bounds.y + 6.0, text_color);
            }
            let _ = has_icon;
        }
        WidgetPayload::Switch { signal } => {
            let on = signal.try_get().unwrap_or(false);
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(11.0).scale(scale),
                fill: if on {
                    theme.switch_on
                } else {
                    theme.switch_off
                },
                ..Default::default()
            });
            let tx = if on { bounds.x + 18.0 } else { bounds.x + 2.0 };
            pctx.ctx.push_sdf(SdfQuad {
                rect: Rect::new(tx, bounds.y + 2.0, 18.0, 18.0).scale(scale),
                radii: CornerRadii::all(9.0).scale(scale),
                fill: theme.control_thumb,
                border_color: theme.hairline,
                border_width: 1.0 * scale,
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });
        }
        WidgetPayload::Slider { signal, min, max } => {
            let val = signal.try_get().unwrap_or(*min);
            let frac = ((val - *min) / (*max - *min).max(0.0001)).clamp(0.0, 1.0);
            let track = Rect::new(
                bounds.x,
                bounds.y + (bounds.height - 4.0) * 0.5,
                bounds.width,
                4.0,
            );
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(track, scale),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.control_track,
                ..Default::default()
            });
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(
                    Rect::new(track.x, track.y, track.width * frac, 4.0),
                    scale,
                ),
                radii: CornerRadii::all(2.0).scale(scale),
                fill: theme.accent,
                ..Default::default()
            });
            let kx = track.x + track.width * frac - 8.0;
            let ky = bounds.y + (bounds.height - 16.0) * 0.5;
            pctx.ctx.push_sdf(SdfQuad {
                rect: Rect::new(kx, ky, 16.0, 16.0).scale(scale),
                radii: CornerRadii::all(8.0).scale(scale),
                fill: theme.control_thumb,
                border_color: theme.hairline,
                border_width: 1.0 * scale,
                shadow_color: theme.shadow_elevated,
                shadow_offset: [0.0, 1.5 * scale],
                shadow_blur: 4.0 * scale,
                ..Default::default()
            });
        }
        WidgetPayload::TextInput {
            signal,
            material,
            cursor,
            selection,
            text_block,
            placeholder_block,
            char_offsets,
            ..
        } => {
            let val = signal.try_get().unwrap_or_default();
            render_material(pctx.ctx, bounds, *material, 8.0, scale);

            if is_focused {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(8.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    border_color: theme.accent,
                    border_width: 2.0 * scale,
                    ..Default::default()
                });
            }

            let inner_clip = Rect::new(
                bounds.x + 8.0,
                bounds.y,
                (bounds.width - 16.0).max(1.0),
                bounds.height,
            );
            pctx.ctx.push_clip(inner_clip);

            let text_x = bounds.x + 12.0;
            let text_y = bounds.y + 8.0;

            if let Some((start, end)) = *selection {
                let s = snap_char_boundary(&val, start.min(end));
                let e = snap_char_boundary(&val, start.max(end));
                if s != e {
                    let x1 = _lookup_offset_x(char_offsets, s);
                    let x2 = _lookup_offset_x(char_offsets, e);
                    let sel_rect = Rect::new(text_x + x1, text_y - 1.0, (x2 - x1).max(2.0), 18.0);
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(sel_rect, scale),
                        radii: CornerRadii::all(3.0).scale(scale),
                        fill: Color::rgba(theme.accent.r, theme.accent.g, theme.accent.b, 0.28),
                        ..Default::default()
                    });
                }
            }

            if val.is_empty() {
                pctx.ctx
                    .push_text_colored(placeholder_block, text_x, text_y, theme.text_tertiary);
            } else {
                pctx.ctx
                    .push_text_colored(text_block, text_x, text_y, theme.text_primary);
            }

            if is_focused && pctx.caret_visible {
                let safe_cursor = snap_char_boundary(&val, *cursor);
                let caret_offset_x = _lookup_offset_x(char_offsets, safe_cursor);
                let caret_x = text_x + caret_offset_x;
                let caret_rect = Rect::new(caret_x, text_y + 1.0, 1.5, 16.0);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }

            pctx.ctx.pop_clip();
        }
        WidgetPayload::TextArea {
            signal,
            material,
            cursor,
            selection,
            text_block,
            placeholder_block,
            scroll_y,
            ..
        } => {
            let val = signal.try_get().unwrap_or_default();
            render_material(pctx.ctx, bounds, *material, 8.0, scale);

            if is_focused {
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(8.0).scale(scale),
                    fill: Color::TRANSPARENT,
                    border_color: theme.accent,
                    border_width: 2.0 * scale,
                    ..Default::default()
                });
            }

            let inner_clip = Rect::new(
                bounds.x + 4.0,
                bounds.y + 4.0,
                (bounds.width - 8.0).max(1.0),
                (bounds.height - 8.0).max(1.0),
            );
            pctx.ctx.push_clip(inner_clip);

            let text_x = bounds.x + 10.0;
            let text_y = bounds.y + 10.0 - *scroll_y;

            if let Some((start, end)) = *selection {
                let s = snap_char_boundary(&val, start.min(end));
                let e = snap_char_boundary(&val, start.max(end));
                text_block.collect_selection_rects(s, e, pctx.selection_scratch);
                for i in 0..pctx.selection_scratch.len() {
                    let (sx, sy, sw) = pctx.selection_scratch[i];
                    let sel_rect = Rect::new(text_x + sx, text_y + sy, sw, 18.0);
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(sel_rect, scale),
                        radii: CornerRadii::all(3.0).scale(scale),
                        fill: Color::rgba(theme.accent.r, theme.accent.g, theme.accent.b, 0.28),
                        ..Default::default()
                    });
                }
            }

            if val.is_empty() {
                pctx.ctx
                    .push_text_colored(placeholder_block, text_x, text_y, theme.text_tertiary);
            } else {
                pctx.ctx
                    .push_text_colored(text_block, text_x, text_y, theme.text_primary);
            }

            if is_focused && pctx.caret_visible {
                let safe_cursor = snap_char_boundary(&val, *cursor);
                let (cy, cx) = text_block.caret_pos_for_byte(safe_cursor);
                let caret_rect = Rect::new(text_x + cx, text_y + cy + 1.0, 1.5, 16.0);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(caret_rect, scale),
                    fill: theme.accent,
                    ..Default::default()
                });
            }

            pctx.ctx.pop_clip();
        }
        WidgetPayload::Tabs {
            items,
            labels,
            label_widths,
            material,
            pill_spring,
            ..
        } => {
            render_material(pctx.ctx, bounds, *material, 7.0, scale);

            let count = items.len().max(1);
            let tab_w = (bounds.width - 4.0) / count as f32;
            let pill_x = bounds.x + 2.0 + pill_spring.value * tab_w;
            let pill_rect = Rect::new(pill_x, bounds.y + 2.0, tab_w, bounds.height - 4.0);

            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(pill_rect, scale),
                radii: CornerRadii::all(5.5).scale(scale),
                fill: theme.control_thumb,
                border_color: theme.hairline,
                border_width: 0.5 * scale,
                shadow_color: theme.shadow_soft,
                shadow_offset: [0.0, 1.0 * scale],
                shadow_blur: 3.0 * scale,
                ..Default::default()
            });

            for (i, label) in labels.iter().enumerate() {
                let item_cx = bounds.x + 2.0 + (i as f32 + 0.5) * tab_w;
                let lw = label_widths.get(i).copied().unwrap_or(0.0);
                pctx.ctx.push_text_colored(
                    label,
                    item_cx - lw * 0.5,
                    bounds.y + 6.0,
                    theme.text_primary,
                );
            }
        }
        WidgetPayload::Dropdown {
            signal,
            items,
            labels,
            material,
            is_open,
            chevron,
            ..
        } => {
            render_material(pctx.ctx, bounds, *material, 8.0, scale);
            let selected = signal
                .try_get()
                .unwrap_or(0)
                .min(items.len().saturating_sub(1));
            if let Some(lbl) = labels.get(selected) {
                pctx.ctx.push_text_colored(
                    lbl,
                    bounds.x + 12.0,
                    bounds.y + 7.0,
                    theme.text_primary,
                );
            }
            pctx.ctx.push_text_colored(
                chevron,
                bounds.x + bounds.width - 20.0,
                bounds.y + 8.0,
                theme.text_secondary,
            );

            if *is_open {
                pctx.overlays.push(OverlayRenderRecord {
                    source_id: wid,
                    trigger_bounds: bounds,
                    selected,
                    item_count: items.len(),
                });
            }
        }
        WidgetPayload::CustomGpuSurface {
            radius,
            animate_signal,
            needs_redraw,
            resolution_scale,
            state,
            ..
        } => {
            let snapped = snap_rect_to_pixels(bounds, scale);
            let pw = ((snapped.width * *resolution_scale).round() as u32).max(1);
            let ph = ((snapped.height * *resolution_scale).round() as u32).max(1);
            let mut st = state.borrow_mut();

            let gpu = pctx.ctx.gpu_mut();
            if st.size != (pw, ph) || st.handle.is_none() {
                if let Some(old_handle) = st.handle.take() {
                    gpu.release_texture(old_handle);
                }
                let (view, handle) = gpu.create_render_texture(pw, ph, ATLAS_FORMAT);
                st.view = Some(view);
                st.handle = Some(handle);
                st.size = (pw, ph);
                needs_redraw.set(true);
            }

            let should_render = needs_redraw.get()
                || animate_signal
                    .as_ref()
                    .is_some_and(|s| s.try_get().unwrap_or(false));
            let view = st.view.clone();
            if let (Some(view), Some(handle)) = (view, st.handle) {
                if should_render {
                    let mut encoder =
                        gpu.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Cadmium CustomGpuSurface Encoder"),
                            });
                    st.pass.render(
                        &gpu.device,
                        &gpu.queue,
                        &mut encoder,
                        &view,
                        ATLAS_FORMAT,
                        pw,
                        ph,
                        pctx.time_secs,
                    );
                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    needs_redraw.set(false);
                }

                pctx.ctx.push_image(ImageQuad {
                    rect: snapped,
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    radii: CornerRadii::all(radius.get()).scale(scale),
                    tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                    texture: handle,
                });
            }
        }
        WidgetPayload::SplitPane {
            direction,
            divider_id,
            divider_color,
            ..
        } => {
            for &child in &node.children {
                paint_node(child, nodes, pctx, offset);
            }
            if let Some(div) = nodes.get(*divider_id) {
                let db = Rect::new(
                    div.bounds.x + offset.0,
                    div.bounds.y + offset.1,
                    div.bounds.width,
                    div.bounds.height,
                );
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(db, scale),
                    fill: divider_color.unwrap_or(theme.hairline),
                    ..Default::default()
                });
                if is_hovered {
                    let pill = match direction {
                        SplitDirection::Horizontal => {
                            Rect::new(db.x - 1.5, db.y + db.height * 0.5 - 18.0, 4.0, 36.0)
                        }
                        SplitDirection::Vertical => {
                            Rect::new(db.x + db.width * 0.5 - 18.0, db.y - 1.5, 36.0, 4.0)
                        }
                    };
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: snap_rect_to_pixels(pill, scale),
                        radii: CornerRadii::all(2.0).scale(scale),
                        fill: theme.accent,
                        ..Default::default()
                    });
                }
            }
        }
        WidgetPayload::VirtualizedList { state } => {
            let mut st = state.borrow_mut();
            let viewport = bounds;
            let stride = st.item_height + st.gap;
            let total_h = if st.item_count == 0 {
                0.0
            } else {
                st.item_count as f32 * stride - st.gap
            };
            let scroll_val = st
                .scroll_y
                .value
                .clamp(0.0, (total_h - viewport.height).max(0.0));

            pctx.ctx.push_clip(viewport);

            if st.item_count > 0 && viewport.height > 0.0 {
                let first = (scroll_val / stride).floor().max(0.0) as usize;
                let visible_n = (viewport.height / stride).ceil() as usize + 2;
                let last = (first + visible_n).min(st.item_count);
                let selected_idx = st.selected.and_then(|s| s.try_get());
                let hovered_idx = st.hovered_row;
                let item_h = st.item_height;
                let pool_len = st.pool.len().max(1);

                for idx in first..last {
                    let slot_idx = idx % pool_len;
                    let needs_build = st.pool[slot_idx].cached_index != Some(idx);
                    if needs_build {
                        let row_data = (st.row_builder)(idx);
                        let te = pctx.ctx.text_engine();
                        let slot = &mut st.pool[slot_idx];
                        slot.cached_index = Some(idx);
                        slot.title.set_text(te, &row_data.title);
                        slot.subtitle.set_text(te, &row_data.subtitle);
                        slot.badge.set_text(te, &row_data.badge);
                        slot.badge_width = slot.badge.measure(te, None).0;
                    }

                    let row_y = viewport.y + idx as f32 * stride - scroll_val;
                    let row_rect = Rect::new(viewport.x, row_y, viewport.width - 12.0, item_h);
                    let is_sel = selected_idx == Some(idx);
                    let is_hov = hovered_idx == Some(idx);
                    let active_accent = st.accent.unwrap_or(theme.accent);

                    if st.flat {
                        let fill = if is_sel {
                            Color::rgba(active_accent.r, active_accent.g, active_accent.b, 0.14)
                        } else if is_hov {
                            theme.control_hover
                        } else if idx % 2 == 1 {
                            Color::rgba(
                                theme.text_primary.r,
                                theme.text_primary.g,
                                theme.text_primary.b,
                                0.026,
                            )
                        } else {
                            Color::TRANSPARENT
                        };

                        if fill.a > 0.001 {
                            pctx.ctx.push_sdf(SdfQuad {
                                rect: snap_rect_to_pixels(row_rect, scale),
                                radii: CornerRadii::all(6.0).scale(scale),
                                fill,
                                ..Default::default()
                            });
                        }
                    } else {
                        let fill = if is_sel {
                            theme.accent_soft
                        } else if is_hov {
                            theme.control_hover
                        } else {
                            theme.panel_bg
                        };

                        pctx.ctx.push_sdf(SdfQuad {
                            rect: snap_rect_to_pixels(row_rect, scale),
                            radii: CornerRadii::all(8.0).scale(scale),
                            fill,
                            border_color: if is_sel { active_accent } else { theme.hairline },
                            border_width: 1.0 * scale,
                            ..Default::default()
                        });
                    }

                    // Optional deduplicated row artwork thumbnail
                    let mut text_start_x = row_rect.x + 14.0;
                    if let Some(art_fn) = st.row_art.as_mut() {
                        let art = art_fn(idx);
                        let key = Arc::as_ptr(&art) as usize;
                        let (handle, aw, ah) = if let Some(&cached) = st.art_cache.get(&key) {
                            cached
                        } else {
                            let h = pctx.ctx.gpu_mut().upload_image(&art);
                            let entry = (h, art.width, art.height);
                            st.art_cache.insert(key, entry);
                            entry
                        };
                        let thumb_sz = (item_h - 12.0).clamp(24.0, 38.0);
                        let thumb_rect = Rect::new(
                            row_rect.x + 12.0,
                            row_rect.y + (item_h - thumb_sz) * 0.5,
                            thumb_sz,
                            thumb_sz,
                        );
                        pctx.ctx.push_image(ImageQuad {
                            rect: snap_rect_to_pixels(thumb_rect, scale),
                            uv_rect: cover_uv_rect(aw, ah, thumb_sz, thumb_sz),
                            radii: CornerRadii::all(5.0).scale(scale),
                            tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                            texture: handle,
                        });
                        text_start_x = row_rect.x + 12.0 + thumb_sz + 12.0;
                    }

                    if st.columns_mode {
                        let title_w = (row_rect.width * 0.44 - (text_start_x - row_rect.x)).max(60.0);
                        let artist_x = row_rect.x + row_rect.width * 0.48;
                        let artist_w = (row_rect.width * 0.40 - 16.0).max(40.0);
                        let text_y = row_rect.y + (item_h - 18.0) * 0.5;
                        let slot = &st.pool[slot_idx];

                        pctx.ctx.push_clip(Rect::new(text_start_x, row_rect.y, title_w, item_h));
                        pctx.ctx.push_text_colored(
                            &slot.title,
                            text_start_x,
                            text_y,
                            if is_sel {
                                active_accent
                            } else {
                                theme.text_primary
                            },
                        );
                        pctx.ctx.pop_clip();

                        pctx.ctx.push_clip(Rect::new(artist_x, row_rect.y, artist_w, item_h));
                        pctx.ctx.push_text_colored(
                            &slot.subtitle,
                            artist_x,
                            text_y,
                            theme.text_secondary,
                        );
                        pctx.ctx.pop_clip();

                        pctx.ctx.push_text_colored(
                            &slot.badge,
                            row_rect.x + row_rect.width - slot.badge_width - 14.0,
                            text_y,
                            theme.text_tertiary,
                        );
                    } else {
                        let slot = &st.pool[slot_idx];
                        pctx.ctx.push_text_colored(
                            &slot.title,
                            text_start_x,
                            row_rect.y + 6.0,
                            if is_sel {
                                active_accent
                            } else {
                                theme.text_primary
                            },
                        );
                        pctx.ctx.push_text_colored(
                            &slot.subtitle,
                            text_start_x,
                            row_rect.y + 24.0,
                            theme.text_secondary,
                        );
                        pctx.ctx.push_text_colored(
                            &slot.badge,
                            row_rect.x + row_rect.width - slot.badge_width - 14.0,
                            row_rect.y + (item_h - 16.0) * 0.5,
                            theme.text_secondary,
                        );
                    }
                }
            }

            pctx.ctx.pop_clip();

            if total_h > viewport.height {
                let track_h = (viewport.height - 8.0).max(0.0);
                let thumb_h =
                    ((viewport.height / total_h) * track_h).clamp(24.0_f32.min(track_h), track_h);
                let max_scroll = (total_h - viewport.height).max(1.0);
                let thumb_t = (scroll_val / max_scroll).clamp(0.0, 1.0);
                let thumb_y = viewport.y + 4.0 + thumb_t * (track_h - thumb_h);
                let thumb_rect =
                    Rect::new(viewport.x + viewport.width - 6.0, thumb_y, 4.0, thumb_h);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(thumb_rect, scale),
                    radii: CornerRadii::all(2.0).scale(scale),
                    fill: Color::rgba(
                        theme.text_tertiary.r,
                        theme.text_tertiary.g,
                        theme.text_tertiary.b,
                        0.42,
                    ),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::ScrollView {
            scroll_y,
            content_height,
            content_id,
        } => {
            let viewport = bounds;
            pctx.ctx.push_clip(viewport);

            let child_offset = (offset.0, offset.1 - scroll_y.value);
            paint_node(*content_id, nodes, pctx, child_offset);

            pctx.ctx.pop_clip();

            if *content_height > viewport.height {
                let track_h = (viewport.height - 8.0).max(0.0);
                let thumb_h = ((viewport.height / *content_height) * track_h)
                    .clamp(24.0_f32.min(track_h), track_h);
                let max_scroll = (*content_height - viewport.height).max(1.0);
                let thumb_t = (scroll_y.value / max_scroll).clamp(0.0, 1.0);
                let thumb_y = viewport.y + 4.0 + thumb_t * (track_h - thumb_h);
                let thumb_rect =
                    Rect::new(viewport.x + viewport.width - 6.0, thumb_y, 4.0, thumb_h);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(thumb_rect, scale),
                    radii: CornerRadii::all(2.0).scale(scale),
                    fill: Color::from_rgba8(0, 0, 0, 75),
                    ..Default::default()
                });
            }
        }
        WidgetPayload::TrafficLights { hovered } => {
            let cy = bounds.y + (bounds.height - 12.0) * 0.5;
            let r1 = snap_rect_to_pixels(Rect::new(bounds.x, cy, 12.0, 12.0), scale);
            let r2 = snap_rect_to_pixels(Rect::new(bounds.x + 20.0, cy, 12.0, 12.0), scale);
            let r3 = snap_rect_to_pixels(Rect::new(bounds.x + 40.0, cy, 12.0, 12.0), scale);

            for (rect, fill) in [
                (r1, theme.traffic_close),
                (r2, theme.traffic_minimize),
                (r3, theme.traffic_zoom),
            ] {
                pctx.ctx.push_sdf(SdfQuad {
                    rect,
                    radii: CornerRadii::all(rect.width * 0.5),
                    fill,
                    border_color: theme.traffic_border,
                    border_width: 1.0 * scale,
                    ..Default::default()
                });
            }

            if *hovered {
                let bar_len = 6.2 * scale;
                let bar_thick = 1.35 * scale;
                let bar_r = CornerRadii::all(bar_thick * 0.5);

                let cx1 = r1.x + r1.width * 0.5;
                let cy1 = r1.y + r1.height * 0.5;
                let x_len = 6.6 * scale;
                let red_glyph = Color::from_rgba8(77, 10, 8, 225);
                for rot in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
                    pctx.ctx.push_sdf(SdfQuad {
                        rect: Rect::new(cx1 - x_len * 0.5, cy1 - bar_thick * 0.5, x_len, bar_thick),
                        radii: bar_r,
                        fill: red_glyph,
                        rotation: rot,
                        ..Default::default()
                    });
                }

                let cx2 = r2.x + r2.width * 0.5;
                let cy2 = r2.y + r2.height * 0.5;
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx2 - bar_len * 0.5,
                        cy2 - bar_thick * 0.5,
                        bar_len,
                        bar_thick,
                    ),
                    radii: bar_r,
                    fill: Color::from_rgba8(153, 87, 0, 225),
                    ..Default::default()
                });

                let cx3 = r3.x + r3.width * 0.5;
                let cy3 = r3.y + r3.height * 0.5;
                let green_glyph = Color::from_rgba8(0, 101, 0, 225);
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx3 - bar_len * 0.5,
                        cy3 - bar_thick * 0.5,
                        bar_len,
                        bar_thick,
                    ),
                    radii: bar_r,
                    fill: green_glyph,
                    ..Default::default()
                });
                pctx.ctx.push_sdf(SdfQuad {
                    rect: Rect::new(
                        cx3 - bar_thick * 0.5,
                        cy3 - bar_len * 0.5,
                        bar_thick,
                        bar_len,
                    ),
                    radii: bar_r,
                    fill: green_glyph,
                    ..Default::default()
                });
            }
        }
        WidgetPayload::Divider { color } => {
            pctx.ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                fill: *color,
                ..Default::default()
            });
        }
        WidgetPayload::Spacer => {}
    }
}

/// Iterative DFS: finds the innermost widget with a tooltip under (x, y).
fn find_tooltip_node(
    root: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    start_offset: (f32, f32),
) -> Option<(WidgetId, Rect)> {
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    let mut result: Option<(WidgetId, Rect)> = None;
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );

        if let WidgetPayload::ScrollView {
            scroll_y,
            content_id,
            ..
        } = &node.payload
        {
            if bounds.contains(x, y) {
                stack.push((*content_id, (offset.0, offset.1 - scroll_y.value)));
            }
            continue;
        }

        if let WidgetPayload::Switcher { signal, .. } = &node.payload
            && let Some(&active_child) = node.children.get(signal.try_get().unwrap_or(0))
        {
            stack.push((active_child, offset));
            continue;
        }

        for &child in &node.children {
            stack.push((child, offset));
        }

        if bounds.contains(x, y) && node.tooltip.is_some() {
            result = Some((wid, bounds));
        }
    }
    result
}

/// Iterative DFS: finds the innermost widget with a context menu under (x, y).
fn find_context_menu_node(
    root: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    start_offset: (f32, f32),
) -> Option<WidgetId> {
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    let mut result: Option<WidgetId> = None;
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );

        if let WidgetPayload::ScrollView {
            scroll_y,
            content_id,
            ..
        } = &node.payload
        {
            if bounds.contains(x, y) {
                stack.push((*content_id, (offset.0, offset.1 - scroll_y.value)));
            }
            continue;
        }

        if let WidgetPayload::Switcher { signal, .. } = &node.payload
            && let Some(&active_child) = node.children.get(signal.try_get().unwrap_or(0))
        {
            stack.push((active_child, offset));
            continue;
        }

        for &child in &node.children {
            stack.push((child, offset));
        }

        if bounds.contains(x, y) && !node.context_menu.is_empty() {
            result = Some(wid);
        }
    }
    result
}

/// Iterative DFS: returns screen bounds of a specific target widget.
fn widget_screen_bounds(
    root: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    target: WidgetId,
    start_offset: (f32, f32),
) -> Option<Rect> {
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );
        if wid == target {
            return Some(bounds);
        }
        let child_offset = match &node.payload {
            WidgetPayload::ScrollView { scroll_y, .. } => (offset.0, offset.1 - scroll_y.value),
            _ => offset,
        };
        for &child in &node.children {
            stack.push((child, child_offset));
        }
    }
    None
}

/// Iterative DFS: dispatch a scroll delta to the innermost scrollable widget
/// under (cx, cy).  Needs `&mut` access, so we collect the candidate path
/// first, then mutate from the deepest match outward.
fn dispatch_scroll_node(
    root: WidgetId,
    nodes: &mut SlotMap<WidgetId, WidgetNode>,
    cx: f32,
    cy: f32,
    delta_y: f32,
    start_offset: (f32, f32),
) -> bool {
    // Phase 1: find the deepest node under the cursor, recording the path with
    // per-node bounds so we can scroll the innermost scrollable widget.
    let mut path: Vec<(WidgetId, Rect)> = Vec::new();
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );
        if !bounds.contains(cx, cy) {
            continue;
        }
        path.push((wid, bounds));

        let child_offset = match &node.payload {
            WidgetPayload::ScrollView { scroll_y, .. } => (offset.0, offset.1 - scroll_y.value),
            _ => offset,
        };
        if let WidgetPayload::Switcher { signal, .. } = &node.payload
            && let Some(&active_child) = node.children.get(signal.try_get().unwrap_or(0))
        {
            stack.push((active_child, child_offset));
            continue;
        }
        // Push children in reverse so the topmost child is popped first.
        for &child in node.children.iter().rev() {
            stack.push((child, child_offset));
        }
    }

    // Phase 2: try to scroll from innermost outward.
    for (wid, bounds) in path.into_iter().rev() {
        let Some(node) = nodes.get_mut(wid) else {
            continue;
        };
        match &mut node.payload {
            WidgetPayload::VirtualizedList { state } => {
                let mut st = state.borrow_mut();
                let stride = st.item_height + st.gap;
                let total_h = if st.item_count == 0 {
                    0.0
                } else {
                    st.item_count as f32 * stride - st.gap
                };
                let max_scroll = (total_h - bounds.height).max(0.0);
                if max_scroll > 0.0 {
                    let next = (st.scroll_y.target - delta_y).clamp(0.0, max_scroll);
                    st.scroll_y.set_target(next);
                    return true;
                }
            }
            WidgetPayload::ScrollView {
                scroll_y,
                content_height,
                ..
            } => {
                let max_scroll = (*content_height - bounds.height).max(0.0);
                if max_scroll > 0.0 {
                    let next = (scroll_y.target - delta_y).clamp(0.0, max_scroll);
                    scroll_y.set_target(next);
                    return true;
                }
            }
            WidgetPayload::TextArea {
                text_block,
                scroll_y,
                ..
            } => {
                let line_count = text_block.line_start_offsets().len().max(1);
                let content_h = line_count as f32 * text_block.style().line_height;
                let max_scroll = (content_h - (bounds.height - 20.0)).max(0.0);
                if max_scroll > 0.0 {
                    *scroll_y = (*scroll_y - delta_y).clamp(0.0, max_scroll);
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Iterative post-order hit-test: finds the topmost (last-painted) interactive
/// widget that contains (x, y).  Accounts for `ScrollView` offset.
fn hit_test_node(
    root: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    start_offset: (f32, f32),
) -> Option<WidgetId> {
    // Stack entries: (widget_id, accumulated_offset)
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    // We want the topmost (last-painted = deepest in z-order) hit.
    // Paint visits children front-to-back, so we process children in reverse
    // push order and return the first matching leaf.
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );

        // Clipped containers reject hits outside their bounds, including children.
        if node.layout_style.clip && !bounds.contains(x, y) {
            continue;
        }

        // SplitPane: check the drag divider zone first.
        if let WidgetPayload::SplitPane {
            direction,
            divider_id,
            ..
        } = &node.payload
            && let Some(div) = nodes.get(*divider_id)
        {
            let div_bounds = Rect::new(
                div.bounds.x + offset.0,
                div.bounds.y + offset.1,
                div.bounds.width,
                div.bounds.height,
            );
            let hit_zone = match direction {
                SplitDirection::Horizontal => {
                    Rect::new(div_bounds.x - 3.5, div_bounds.y, 8.0, div_bounds.height)
                }
                SplitDirection::Vertical => {
                    Rect::new(div_bounds.x, div_bounds.y - 3.5, div_bounds.width, 8.0)
                }
            };
            if hit_zone.contains(x, y) {
                return Some(wid);
            }
        }

        // ScrollView: clip test + offset adjustment for content.
        if let WidgetPayload::ScrollView {
            scroll_y,
            content_id,
            ..
        } = &node.payload
        {
            if !bounds.contains(x, y) {
                continue;
            }
            stack.push((*content_id, (offset.0, offset.1 - scroll_y.value)));
            continue;
        }

        if let WidgetPayload::Switcher { signal, .. } = &node.payload
            && let Some(&active_child) = node.children.get(signal.try_get().unwrap_or(0))
        {
            stack.push((active_child, offset));
            continue;
        }

        // Push children in forward order so the last child (topmost) is
        // processed first (LIFO stack = reverse iteration order).
        for &child in &node.children {
            stack.push((child, offset));
        }

        // Leaf check: is this node interactive and under the cursor?
        if bounds.contains(x, y)
            && matches!(
                node.payload,
                WidgetPayload::Button { .. }
                    | WidgetPayload::Switch { .. }
                    | WidgetPayload::Slider { .. }
                    | WidgetPayload::TextInput { .. }
                    | WidgetPayload::TextArea { .. }
                    | WidgetPayload::Tabs { .. }
                    | WidgetPayload::Dropdown { .. }
                    | WidgetPayload::TrafficLights { .. }
                    | WidgetPayload::VirtualizedList { .. }
                    | WidgetPayload::Image {
                        on_click: Some(_),
                        ..
                    }
                    | WidgetPayload::Container {
                        on_click: Some(_),
                        ..
                    }
            )
        {
            return Some(wid);
        }
    }
    None
}

/// Iterative search: finds the topmost `Container` with `is_drag_region` that
/// contains (x, y).
fn is_drag_node(
    root: WidgetId,
    nodes: &SlotMap<WidgetId, WidgetNode>,
    x: f32,
    y: f32,
    start_offset: (f32, f32),
) -> bool {
    let mut stack: Vec<(WidgetId, (f32, f32))> = vec![(root, start_offset)];
    while let Some((wid, offset)) = stack.pop() {
        let Some(node) = nodes.get(wid) else { continue };
        let bounds = Rect::new(
            node.bounds.x + offset.0,
            node.bounds.y + offset.1,
            node.bounds.width,
            node.bounds.height,
        );
        if let WidgetPayload::Container {
            is_drag_region: true,
            ..
        } = &node.payload
            && bounds.contains(x, y)
        {
            return true;
        }
        for &child in &node.children {
            stack.push((child, offset));
        }
    }
    false
}

fn render_material(
    ctx: &mut FrameContext<'_>,
    bounds: Rect,
    material: Material,
    radius: f32,
    scale: f32,
) {
    match material {
        Material::Classic {
            fill,
            fill_end,
            gradient_dir,
            border_color,
            border_width,
            shadow,
        } => {
            let (shad_col, shad_blur, shad_off) = match shadow {
                Some(s) => (
                    s.color,
                    s.blur * scale,
                    [s.offset[0] * scale, s.offset[1] * scale],
                ),
                None => (Color::TRANSPARENT, 0.0, [0.0, 0.0]),
            };
            ctx.push_sdf(SdfQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                fill,
                fill_end,
                gradient_dir,
                rotation: 0.0,
                border_color,
                border_width: border_width * scale,
                shadow_color: shad_col,
                shadow_offset: shad_off,
                shadow_blur: shad_blur,
            });
        }
        Material::Frosted {
            tint,
            frost,
            specular,
            border_color,
            border_width,
            shadow,
        } => {
            if let Some(s) = shadow {
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: s.color,
                    shadow_offset: [s.offset[0] * scale, s.offset[1] * scale],
                    shadow_blur: s.blur * scale,
                    ..Default::default()
                });
            }
            ctx.push_glass(GlassQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                tint,
                border_color,
                border_width: border_width * scale,
                saturation: 1.35,
                specular,
                frost,
                liquid_ripple: 0.0,
                ..Default::default()
            });
        }
        Material::Liquid {
            tint,
            frost,
            ripple,
            specular,
            rim_angle,
            sheen_progress,
            conic_strength,
            optics,
            border_color,
            border_width,
            shadow,
        } => {
            if let Some(s) = shadow {
                ctx.push_sdf(SdfQuad {
                    rect: snap_rect_to_pixels(bounds, scale),
                    radii: CornerRadii::all(radius).scale(scale),
                    fill: Color::TRANSPARENT,
                    shadow_color: s.color,
                    shadow_offset: [s.offset[0] * scale, s.offset[1] * scale],
                    shadow_blur: s.blur * scale,
                    ..Default::default()
                });
            }
            ctx.push_glass(GlassQuad {
                rect: snap_rect_to_pixels(bounds, scale),
                radii: CornerRadii::all(radius).scale(scale),
                tint,
                border_color,
                border_width: border_width * scale,
                saturation: 1.25,
                specular,
                frost,
                liquid_ripple: ripple,
                rim_angle,
                sheen_progress,
                conic_strength,
                optics,
            });
        }
    }
}

