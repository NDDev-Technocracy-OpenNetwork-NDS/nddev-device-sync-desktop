import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';

// Presentation owns one current image and one decode. Rust owns credentials,
// native receive/decode, cancellation, original expiry and retained tickets.
class ScreenViewer extends StatefulWidget {
  const ScreenViewer({
    super.key,
    required this.client,
    required this.deviceId,
    this.organization,
  });
  final native.NdsClient client;
  final String deviceId;
  final native.LocalOrganization? organization;
  @override
  State<ScreenViewer> createState() => _ScreenViewerState();
}

class _Image {
  _Image(this.image, this.ticket);
  final ui.Image image;
  final native.ScreenFrame ticket;
  void dispose() {
    image.dispose();
    ticket.dispose();
  }
}

class _ScreenViewerState extends State<ScreenViewer> {
  native.ScreenView? _view;
  _Image? _current;
  ApplicationActivity? _activity;
  int _epoch = 0;
  bool _opening = true;
  bool _disposed = false;
  bool _stopped = false;
  Object? _error;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final activity = ApplicationActivityScope.of(context);
    if (_activity != activity) {
      _activity?.removeListener(_activityChanged);
      _activity = activity;
      activity.addListener(_activityChanged);
      if (_opening) {
        _opening = false;
        unawaited(_open());
      }
    }
  }

  void _activityChanged() {
    if (_activity?.value != true) unawaited(_stop());
  }

  Future<void> _open() async {
    final epoch = _epoch;
    try {
      final organization = widget.organization;
      final view = organization == null
          ? await widget.client.screenOpen(target: widget.deviceId)
          : await widget.client.screenOpenForOrganization(
              organization: organization,
              target: widget.deviceId,
            );
      if (!mounted || epoch != _epoch || _activity?.value != true) {
        await view.close();
        view.dispose();
        return;
      }
      _view = view;
      unawaited(_closed(view, epoch));
      await _frames(view, epoch);
    } catch (error) {
      if (mounted && epoch == _epoch) setState(() => _error = error);
      await _stop();
    }
  }

  Future<void> _closed(native.ScreenView view, int epoch) async {
    await view.closed();
    if (mounted && epoch == _epoch) await _stop();
  }

  Future<void> _frames(native.ScreenView view, int epoch) async {
    while (mounted && epoch == _epoch && !_stopped) {
      native.ScreenFrame? ticket;
      ui.ImmutableBuffer? buffer;
      ui.ImageDescriptor? descriptor;
      ui.Codec? codec;
      ui.Image? image;
      try {
        ticket = await view.frame();
        final layout = ticket.layout();
        final bytes = await ticket.readOnce();
        buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
        descriptor = ui.ImageDescriptor.raw(
          buffer,
          width: layout.width,
          height: layout.height,
          rowBytes: layout.stride,
          pixelFormat: ui.PixelFormat.bgra8888,
        );
        codec = await descriptor.instantiateCodec();
        image = (await codec.getNextFrame()).image;
        // Native ticket remains held through every async renderer boundary.
        await ticket.validateContext();
        ticket.checkLive();
        if (!mounted || epoch != _epoch || _activity?.value != true) return;
        final previous = _current;
        setState(() {
          _current = _Image(image!, ticket!);
          _error = null;
        });
        image = null;
        ticket = null;
        // The previous RawImage detaches before its creator handle/permit leave.
        await WidgetsBinding.instance.endOfFrame;
        previous?.dispose();
      } finally {
        image?.dispose();
        ticket?.dispose();
        codec?.dispose();
        descriptor?.dispose();
        buffer?.dispose();
      }
    }
  }

  Future<void> _stop() async {
    if (_stopped) return;
    _stopped = true;
    _epoch++;
    widget.client.screenCancel();
    final view = _view;
    _view = null;
    final old = _current;
    if (mounted && !_disposed) {
      setState(() => _current = null);
      await WidgetsBinding.instance.endOfFrame;
      old?.dispose();
    } else {
      _current = null;
      if (old != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) => old.dispose());
      }
    }
    if (view != null) {
      try {
        await view.close();
      } finally {
        view.dispose();
      }
    } else {
      await widget.client.screenStop();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _activity?.removeListener(_activityChanged);
    unawaited(_stop());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l.screenView)),
      body: SafeArea(
        child: Column(
          children: [
            if (_current != null)
              Expanded(
                child: Center(
                  child: RawImage(image: _current!.image, fit: BoxFit.contain),
                ),
              ),
            if (_current == null)
              Text(
                _error == null
                    ? (_stopped ? l.screenEnded : l.screenOpening)
                    : l.screenUnavailable,
              ),
            TextButton(
              onPressed: _stopped ? null : () => unawaited(_stop()),
              child: Text(l.screenStop),
            ),
          ],
        ),
      ),
    );
  }
}
