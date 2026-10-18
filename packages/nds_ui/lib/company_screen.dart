import 'dart:async';

import 'package:flutter/material.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';
import 'screen_targets.dart';

/// Presentation only. Rust retains immutable commands; the server authorizes
/// every page and mutation, including a retry after an uncertain response.
class CompanyScreen extends StatefulWidget {
  const CompanyScreen({super.key, required this.client});
  final native.NdsClient client;
  @override
  State<CompanyScreen> createState() => _CompanyState();
}

class _CompanyState extends State<CompanyScreen> {
  native.OrganizationOverviewView? _company;
  native.OrganizationMemberPageView? _members;
  native.OrganizationInvitationPageView? _invitations;
  native.CompanyCommand? _command;
  final _email = TextEditingController();
  native.OrganizationRoleView _role = native.OrganizationRoleView.user;
  bool _busy = false;
  String? _error;
  bool _completed = false;
  int _emailEpoch = 0;
  late final String _scope;
  Timer? _watch;
  bool get _paused => !ApplicationActivityScope.read(context).value;
  String get _currentScope {
    final view = widget.client.snapshot();
    return view.authenticated
        ? '${view.tenantId}/${view.userId}/${view.currentDeviceId}'
        : '';
  }

  @override
  void initState() {
    super.initState();
    _scope = _currentScope;
    _watch = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted && _currentScope != _scope) {
        setState(() {
          _company = null;
          _members = null;
          _invitations = null;
        });
        if (ModalRoute.of(context)?.isCurrent == true) {
          _watch?.cancel();
          Navigator.of(context).pop();
        }
      }
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_run(_load));
    });
  }

  @override
  void dispose() {
    _watch?.cancel();
    _command?.dispose();
    _email.clear();
    _email.dispose();
    super.dispose();
  }

  Future<void> _run(Future<void> Function() work) async {
    if (_busy || _paused || _currentScope != _scope) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await work();
    } catch (error) {
      var code = 'unexpected';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      }
      if (mounted && _currentScope == _scope) {
        setState(() {
          _error = code;
          // Do not retain an administrative page after access is denied.
          if (code == 'authorization_denied' ||
              code == 'authentication_failed') {
            _members = null;
            _invitations = null;
            _company = null;
          }
        });
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _load() async {
    final company = await widget.client.company();
    if (!mounted || _currentScope != _scope) return;
    if (company.member.role != native.OrganizationRoleView.admin) {
      setState(() {
        _company = company;
        _members = null;
        _invitations = null;
      });
      return;
    }
    final members = await widget.client.companyMembers();
    final invitations = await widget.client.companyInvitations();
    if (!mounted || _currentScope != _scope) return;
    // Replace the completed read together. Collapsing the list between requests
    // resets a mobile viewport; denied access is cleared by the error boundary.
    setState(() {
      _company = company;
      _members = members;
      _invitations = invitations;
    });
  }

  Future<void> _screens(native.OrganizationMemberView member) async {
    await Navigator.of(context).push<void>(
      MaterialPageRoute<void>(
        builder: (_) =>
            ScreenTargets(client: widget.client, userId: member.userId),
      ),
    );
    if (mounted) await _run(_load);
  }

  Future<void> _execute() async {
    final command = _command!;
    await command.execute();
    if (!mounted) return;
    command.dispose();
    _command = null;
    if (!mounted || _currentScope != _scope) return;
    setState(() {
      _completed = true;
      _emailEpoch++;
    });
    _email.clear();
    // A failed refresh must not turn a committed result into an uncertain one.
    await _load();
  }

  Future<void> _submit(Future<native.CompanyCommand> Function() prepare) =>
      _run(() async {
        if (_command != null) return;
        _completed = false;
        final command = await prepare();
        if (!mounted || _currentScope != _scope) {
          command.dispose();
          return;
        }
        _command = command;
        await _execute();
      });

  Future<void> _change(
    native.OrganizationMemberView member,
    native.OrganizationRoleView? role,
  ) async {
    final l = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(role == null ? l.companyRemove : l.companyChangeRole),
        content: Text(l.companyChangeHelp(member.email ?? member.userId)),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.companyConfirm),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    await _submit(
      () => widget.client.companyChangeMember(
        user: member.userId,
        revision: member.revision,
        role: role,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    ApplicationActivityScope.of(context);
    final l = AppLocalizations.of(context)!;
    final company = _company;
    final enabled = !_busy && !_paused && _command == null;
    String role(native.OrganizationRoleView value) =>
        value == native.OrganizationRoleView.admin
        ? l.companyAdmin
        : l.companyUser;
    return Scaffold(
      appBar: AppBar(title: Text(l.companyTitle)),
      body: ListView(
        padding: const EdgeInsets.all(OpenNetworkSpacing.six),
        children: [
          Text(l.companyPrivacy),
          if (company != null) ...[
            Text(
              company.name,
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            Text(role(company.member.role)),
          ],
          if (_completed)
            Semantics(liveRegion: true, child: Text(l.companyCompleted)),
          if (_error != null)
            Semantics(
              liveRegion: true,
              child: Text(switch (_error) {
                'last_administrator' => l.companyLastAdmin,
                'operation_conflict' => l.companyConflict,
                'command_expired' => l.companyExpired,
                'invitation_unavailable' => l.companyInvitationUnavailable,
                'authorization_denied' => l.companyDenied,
                'invalid_input' || 'invalid_request' => l.companyInvalid,
                'busy' ||
                'server_busy' ||
                'rate_limited' ||
                'online_verification_required' => l.operationBusy,
                'transport_unavailable' ||
                'request_timeout' ||
                'dependency_unavailable' => l.networkError,
                'authentication_failed' ||
                'device_revoked' ||
                'credential_revalidation_required' => l.authError,
                _ => l.genericError,
              }, key: const Key('company-error')),
            ),
          if (_command != null) ...[
            Text(l.companyPending),
            FilledButton(
              onPressed: _busy || _paused ? null : () => _run(_execute),
              child: Text(l.companyRetry),
            ),
            TextButton(
              onPressed: _busy || _paused
                  ? null
                  : () => _run(() async {
                      // Deliberate review of authoritative state, never an automatic new command.
                      await _load();
                      _command?.dispose();
                      _command = null;
                    }),
              child: Text(l.companyReview),
            ),
          ],
          TextButton(
            onPressed: _busy || _paused ? null : () => _run(_load),
            child: Text(l.refresh),
          ),
          if (company?.member.role == native.OrganizationRoleView.admin) ...[
            Text(
              l.companyMembers,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            for (final member
                in _members?.members ?? <native.OrganizationMemberView>[])
              Card(
                child: ListTile(
                  title: Text(member.email ?? member.userId),
                  subtitle: Text(
                    '${role(member.role)} · ${member.active ? l.companyActive : l.companyRemoved}',
                  ),
                  trailing: member.active
                      ? PopupMenuButton<String>(
                          enabled: enabled,
                          onSelected: (action) {
                            if (action == 'screens') {
                              unawaited(_screens(member));
                              return;
                            }
                            unawaited(
                              _change(member, switch (action) {
                                'admin' => native.OrganizationRoleView.admin,
                                'user' => native.OrganizationRoleView.user,
                                _ => null,
                              }),
                            );
                          },
                          itemBuilder: (_) => [
                            if (native.screenSupported())
                              PopupMenuItem(
                                value: 'screens',
                                child: Text(l.screenView),
                              ),
                            PopupMenuItem(
                              value: 'admin',
                              child: Text(l.companyAdmin),
                            ),
                            PopupMenuItem(
                              value: 'user',
                              child: Text(l.companyUser),
                            ),
                            PopupMenuItem(
                              value: 'remove',
                              child: Text(l.companyRemove),
                            ),
                          ],
                        )
                      : null,
                ),
              ),
            if (_members?.nextCursor case final String next)
              TextButton(
                onPressed: enabled
                    ? () => _run(() async {
                        final page = await widget.client.companyMembers(
                          after: next,
                        );
                        if (mounted && _currentScope == _scope) {
                          setState(() => _members = page);
                        }
                      })
                    : null,
                child: Text(l.companyNextMembers),
              ),
            const SizedBox(height: OpenNetworkSpacing.six),
            Text(
              l.companyInvite,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            KeyedSubtree(
              key: ValueKey(_emailEpoch),
              child: TextField(
                key: const Key('company-email'),
                controller: _email,
                enabled: enabled,
                keyboardType: TextInputType.emailAddress,
                autocorrect: false,
                decoration: InputDecoration(labelText: l.email),
              ),
            ),
            DropdownButton<native.OrganizationRoleView>(
              value: _role,
              onChanged: enabled
                  ? (value) => setState(() => _role = value!)
                  : null,
              items: [
                for (final value in native.OrganizationRoleView.values)
                  DropdownMenuItem(value: value, child: Text(role(value))),
              ],
            ),
            FilledButton(
              key: const Key('company-invite'),
              onPressed: enabled
                  ? () => _submit(
                      () => widget.client.companyInvite(
                        email: _email.text.trim(),
                        role: _role,
                      ),
                    )
                  : null,
              child: Text(l.companyInvite),
            ),
            Text(l.companyDeliveryHelp),
            for (final invitation
                in _invitations?.invitations ??
                    <native.OrganizationInvitationView>[])
              Card(
                child: ListTile(
                  title: Text(invitation.email),
                  subtitle: Text(
                    '${role(invitation.role)} · ${switch (invitation.status) {
                      native.OrganizationInvitationStatusView.pending => l.companyInvitePending,
                      native.OrganizationInvitationStatusView.accepted => l.companyJoined,
                      native.OrganizationInvitationStatusView.expired => l.companyInviteExpired,
                      native.OrganizationInvitationStatusView.revoked => l.companyInviteRevoked,
                    }}\n${switch (invitation.delivery) {
                      native.OrganizationDeliveryView.pending => l.companyDeliveryPending,
                      native.OrganizationDeliveryView.accepted => l.companyDeliveryAccepted,
                      native.OrganizationDeliveryView.failed => l.companyDeliveryFailed,
                      native.OrganizationDeliveryView.unknown => l.companyDeliveryUnknown,
                    }}',
                  ),
                  trailing:
                      invitation.status ==
                          native.OrganizationInvitationStatusView.pending
                      ? TextButton(
                          onPressed: enabled
                              ? () => _submit(
                                  () => widget.client.companyRevoke(
                                    invitation: invitation.id,
                                  ),
                                )
                              : null,
                          child: Text(l.companyRevoke),
                        )
                      : null,
                ),
              ),
            if (_invitations?.nextCursor case final String next)
              TextButton(
                onPressed: enabled
                    ? () => _run(() async {
                        final page = await widget.client.companyInvitations(
                          after: next,
                        );
                        if (mounted && _currentScope == _scope) {
                          setState(() => _invitations = page);
                        }
                      })
                    : null,
                child: Text(l.companyNextInvitations),
              ),
          ],
          if (_busy) const LinearProgressIndicator(),
        ],
      ),
    );
  }
}
