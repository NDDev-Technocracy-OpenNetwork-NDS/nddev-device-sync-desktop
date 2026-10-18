import 'dart:async';

import 'package:flutter/material.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';

// Explicit host actions over the same NdsClient device resource as the viewer.
// Request metadata is displayed; only Accept asks Rust for source/grant admission.
class ScreenRequests extends StatefulWidget {
  const ScreenRequests({super.key, required this.client});
  final native.NdsClient client;
  @override
  State<ScreenRequests> createState() => _ScreenRequestsState();
}

class _ScreenRequestsState extends State<ScreenRequests> {
  native.ScreenRequest? _request;
  native.ScreenHost? _host;
  bool _busy = false;
  bool _disposed = false;
  Object? _error;
  ApplicationActivity? _activity;
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final activity = ApplicationActivityScope.of(context);
    if (_activity != activity) {
      _activity?.removeListener(_changed);
      _activity = activity;
      activity.addListener(_changed);
    }
  }

  void _changed() {
    if (_activity?.value != true) unawaited(_stop());
  }

  Future<void> _refresh() async {
    if (_busy || _activity?.value != true) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await widget.client.screenPrepare();
      final request = await widget.client.screenIncoming();
      if (mounted && !_disposed) setState(() => _request = request);
    } catch (error) {
      if (mounted && !_disposed) setState(() => _error = error);
    } finally {
      if (mounted && !_disposed) setState(() => _busy = false);
    }
  }

  Future<void> _accept() async {
    final request = _request;
    if (_busy || request == null || _activity?.value != true) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final host = await widget.client.screenAccept(id: request.id);
      if (!mounted || _disposed || _activity?.value != true) {
        await host.close();
        host.dispose();
        return;
      }
      setState(() => _host = host);
      unawaited(_closed(host));
    } catch (error) {
      if (mounted && !_disposed) setState(() => _error = error);
    } finally {
      if (mounted && !_disposed) setState(() => _busy = false);
    }
  }

  Future<void> _closed(native.ScreenHost host) async {
    await host.closed();
    if (_host == host) await _stop();
  }

  Future<void> _stop() async {
    final host = _host;
    _host = null;
    widget.client.screenCancel();
    if (mounted && !_disposed) setState(() {});
    if (host != null) {
      try {
        await host.close();
      } finally {
        host.dispose();
      }
    } else {
      await widget.client.screenStop();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _activity?.removeListener(_changed);
    unawaited(_stop());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l.screenRequests)),
      body: SafeArea(
        child: Column(
          children: [
            Text(l.screenRequestsHelp),
            TextButton(
              onPressed: _busy ? null : () => unawaited(_refresh()),
              child: Text(l.moduleRefresh),
            ),
            if (_error != null) Text(l.screenUnavailable),
            if (_request != null) ...[
              Text(_request!.viewerDevice),
              Text(l.screenConsent),
              FilledButton(
                onPressed: _busy || _host != null
                    ? null
                    : () => unawaited(_accept()),
                child: Text(l.screenAllow),
              ),
            ],
            if (_host != null) ...[
              Text(l.screenAccessAllowed),
              TextButton(
                onPressed: () => unawaited(_stop()),
                child: Text(l.screenStopSharing),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
