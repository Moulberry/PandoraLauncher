use block2::RcBlock;
use objc2::rc::Retained as Id;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, Message};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName, NSGradient,
    NSGraphicsContext, NSImage, NSShadow, NSShadowAttributeName,
};
use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGBlendMode, CGColor, CGContext, CGImage, kCGColorBlack};
use objc2_foundation::{NSAttributedString, NSMutableDictionary, NSString};

pub trait NSImageExt {
    fn draw<F>(size: CGSize, draw: F) -> Id<Self>
    where
        F: Fn(CGRect) -> Option<()> + 'static;

    fn text(text: &str, scale: CGFloat, shadow: bool) -> Id<Self>;
    fn stack(back: &Self, front: &Self, stagger: (CGFloat, CGFloat)) -> Id<Self>;
    fn rect(&self) -> CGRect;
    fn etched(&self) -> Id<Self>;
}

impl NSImageExt for NSImage {
    fn draw<F>(size: CGSize, draw: F) -> Id<Self>
    where
        F: Fn(CGRect) -> Option<()> + 'static,
    {
        let block = RcBlock::new(move |rect| draw(rect).is_some().into());
        NSImage::imageWithSize_flipped_drawingHandler(size, false, &block)
    }

    fn text(text: &str, scale: CGFloat, shadow: bool) -> Id<Self> {
        let base = 256.0;
        let margin = base / 256.0 * scale;

        let text = unsafe {
            let attrs = NSMutableDictionary::<NSString, AnyObject>::new();

            let font = NSFont::systemFontOfSize(base * scale);
            attrs.insert(NSFontAttributeName, &font);

            if shadow {
                let shadow = NSShadow::new();
                shadow.setShadowColor(Some(&NSColor::blackColor().colorWithAlphaComponent(0.4)));
                shadow.setShadowBlurRadius(base / 24.0 * scale);
                shadow.setShadowOffset(CGSize::new(0.0, -base / 48.0 * scale));
                attrs.insert(NSShadowAttributeName, &shadow);
            }

            NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs)
        };

        let size = text.size();
        let outer = CGSize::new(size.width + margin * 2.0, size.height + margin * 2.0);
        let image = Self::draw(outer, move |rect| {
            let origin = CGPoint::new(margin, margin);
            let inner = CGRect::new(origin, rect.size);
            text.drawInRect(inner);
            Some(())
        });

        // Redraw the image with CGContext API to deal with the weird shadow behavior
        Self::draw(outer, move |rect| {
            let ctx = NSGraphicsContext::currentContext().unwrap().CGContext();
            let image = CGImage::from_ns_image(&image)?;
            CGContext::draw_image(Some(&ctx), rect, Some(&image));
            Some(())
        })
    }

    fn stack(back: &Self, front: &Self, shift: (CGFloat, CGFloat)) -> Id<Self> {
        let back = back.retain();
        let front = front.retain();

        let size = back.size();
        Self::draw(size, move |rect| {
            back.drawInRect(rect);

            let outer = rect.size;
            let inner = front.size();
            let origin = CGPoint::new(
                (outer.width - inner.width) / 2.0 + outer.width * shift.0 / 100.0,
                (outer.height - inner.height) / 2.0 + outer.height * shift.1 / 100.0,
            );
            front.drawInRect(CGRect::new(origin, inner));

            Some(())
        })
    }

    fn rect(&self) -> CGRect {
        CGRect::new(CGPoint::ZERO, self.size())
    }

    // Greatly inspired by https://stackoverflow.com/a/7138497
    fn etched(&self) -> Id<Self> {
        let this = self.retain();
        let size = self.size();
        Self::draw(size, move |rect| unsafe {
            // Note: assuming that the image is already a mask since it's drawn from text
            // in the only use case of this method
            // If the assumption is no longer true, refer to https://stackoverflow.com/a/8127762
            let mask = CGImage::from_ns_image(&this)?;
            let ctx = NSGraphicsContext::currentContext().unwrap().CGContext();

            // Draw white drop shadow with shifted mask
            let shifted = mask.shift(rect, CGPoint::new(0.0, -3.0))?;
            CGContext::clip_to_mask(Some(&ctx), rect, Some(&shifted));
            CGContext::set_rgb_fill_color(Some(&ctx), 1.0, 1.0, 1.0, 1.0);
            CGContext::fill_rect(Some(&ctx), rect);

            // Draw gradient
            CGContext::clip_to_mask(Some(&ctx), rect, Some(&mask));
            let gradient = NSGradient::initWithStartingColor_endingColor(
                NSGradient::alloc(),
                &NSColor::colorWithDeviceWhite_alpha(0.5, 1.0),
                &NSColor::colorWithDeviceWhite_alpha(0.25, 1.0),
            )?;
            gradient.drawInRect_angle(rect, 90.0);

            // Draw inner shadow with inverted mask
            let offset = CGSize::new(0.0, size.height / 256.0);
            let blur = size.height / 128.0;
            let color = CGColor::constant_color(Some(kCGColorBlack))?;
            let inverted = mask.invert(rect)?;
            CGContext::set_shadow_with_color(Some(&ctx), offset, blur, Some(&color));
            CGContext::draw_image(Some(&ctx), rect, Some(&inverted));

            Some(())
        })
    }
}

trait CGImageExt {
    fn from_ns_image(image: &NSImage) -> Option<Id<Self>>;
    fn invert(&self, rect: CGRect) -> Option<Id<Self>>;
    fn shift(&self, rect: CGRect, delta: CGPoint) -> Option<Id<Self>>;
}

impl CGImageExt for CGImage {
    fn from_ns_image(image: &NSImage) -> Option<Id<Self>> {
        unsafe { image.CGImageForProposedRect_context_hints(&mut image.rect(), None, None) }
    }

    fn invert(&self, rect: CGRect) -> Option<Id<Self>> {
        let this = self.retain();
        let image = NSImage::draw(rect.size, move |rect| {
            let ctx = NSGraphicsContext::currentContext().unwrap().CGContext();

            CGContext::set_blend_mode(Some(&ctx), CGBlendMode::XOR);
            CGContext::draw_image(Some(&ctx), rect, Some(&this));
            CGContext::set_rgb_fill_color(Some(&ctx), 1.0, 1.0, 1.0, 1.0);
            CGContext::fill_rect(Some(&ctx), rect);

            Some(())
        });

        Self::from_ns_image(&image)
    }

    fn shift(&self, rect: CGRect, delta: CGPoint) -> Option<Id<Self>> {
        let this = self.retain();
        let image = NSImage::draw(rect.size, move |rect| {
            let ctx = NSGraphicsContext::currentContext().unwrap().CGContext();
            let rect = CGRect::new(delta, rect.size);
            CGContext::draw_image(Some(&ctx), rect, Some(&this));
            Some(())
        });

        Self::from_ns_image(&image)
    }
}
