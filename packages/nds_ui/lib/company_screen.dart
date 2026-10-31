import 'dart:async';

import 'package:flutter/material.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';
import 'screen_targets.dart';

/// Presentation only. Rust owns the original personal/selected lifetimes and
/// immutable commands; the server checks current membership on every request.
class CompanyScreen extends StatefulWidget {
  const CompanyScreen({super.key, required this.client});
  final native.NdsClient client;
  @override
  State<CompanyScreen> createState() => _CompanyState();
}

class _CompanyState extends State<CompanyScreen> {
  late final native.NdsClient _client;
  late final String _scope;
  List<native.OrganizationOverviewView> _choices = [];
  native.LocalOrganization? _organization;
  String? _selectedId;
  native.OrganizationOverviewView? _company;
  native.OrganizationMemberPageView? _members;
  native.OrganizationInvitationPageView? _invitations;
  native.CompanyCommand? _command;
  final _email = TextEditingController();
  final _name = TextEditingController();
  native.OrganizationRoleView _role = native.OrganizationRoleView.user;
  bool _busy = false;
  bool _leaving = false;
  bool _canPop = false;
  bool _reviewedUnknown = false;
  String? _error;
  bool _completed = false;
  int _emailEpoch = 0;
  int _epoch = 0;
  Timer? _watch;
  Future<void>? _operation;
  DialogRoute<bool>? _confirmation;
  bool get _paused => !ApplicationActivityScope.read(context).value;
  String get _currentScope {
    final view = _client.snapshot();
    return view.authenticated
        ? '${view.tenantId}/${view.userId}/${view.currentDeviceId}'
        : '';
  }

  bool get _samePersonal =>
      mounted && identical(widget.client, _client) && _currentScope == _scope;
  bool _current(int epoch, [native.LocalOrganization? owner]) =>
      _samePersonal &&
      !_leaving &&
      epoch == _epoch &&
      (owner == null || identical(owner, _organization) && owner.isCurrent());

  @override
  void initState() {
    super.initState();
    _client = widget.client;
    _scope = _currentScope;
    _watch = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!_samePersonal) {
        _organization?.close();
        if (mounted) unawaited(_leave());
      } else if (!_leaving &&
          _organization != null &&
          !_organization!.isCurrent()) {
        setState(() {
          _epoch++;
          _retireSelection();
          _choices = [];
          _error = 'authorization_denied';
        });
      }
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_run(_load));
    });
  }

  @override
  void didUpdateWidget(covariant CompanyScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(widget.client, _client)) unawaited(_leave());
  }

  void _retireSelection() {
    _dismissConfirmation();
    _organization?.close();
    _organization?.dispose();
    _organization = null;
    _selectedId = null;
    _email.clear();
    _role = native.OrganizationRoleView.user;
    _company = null;
    _members = null;
    _invitations = null;
  }

  void _dismissConfirmation() {
    final route = _confirmation;
    _confirmation = null;
    if (route == null) return;
    // didUpdateWidget/dispose may run during a frame. Retire only this screen's
    // captured route after it; never pop another page or dialog by presence.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final navigator = route.navigator;
      if (navigator != null && navigator.mounted && route.isActive) {
        navigator.removeRoute<bool>(route, false);
      }
    });
  }

  Future<void> _leave() async {
    if (_leaving) return;
    _leaving = true;
    _epoch++;
    _organization?.close();
    _dismissConfirmation();
    if (mounted) {
      setState(() {
        _choices = [];
        _company = null;
        _members = null;
        _invitations = null;
        _selectedId = null;
        _completed = false;
        _reviewedUnknown = false;
        _error = null;
        _email.clear();
        _name.clear();
      });
    }
    // Normal navigation waits for admitted work. Closing a selection is not a
    // cancellation/rollback acknowledgement for a submitted server command.
    await _operation;
    if (!mounted) return;
    setState(() => _canPop = true);
    await WidgetsBinding.instance.endOfFrame;
    if (mounted && ModalRoute.of(context)?.isCurrent == true) {
      Navigator.of(context).pop();
    }
  }

  @override
  void dispose() {
    _epoch++;
    _watch?.cancel();
    _retireSelection();
    _command?.dispose();
    _email.clear();
    _email.dispose();
    _name.clear();
    _name.dispose();
    super.dispose();
  }

  Future<void> _run(Future<void> Function() work) async {
    if (_busy || _paused || !_samePersonal || _leaving) return;
    final done = Completer<void>();
    _operation = done.future;
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
      if (_samePersonal && !_leaving) {
        setState(() {
          _error = code;
          if (code == 'authorization_denied' ||
              code == 'authentication_failed' ||
              code == 'credential_revalidation_required' ||
              code == 'device_revoked') {
            _epoch++;
            _retireSelection();
            _choices = [];
          }
        });
      }
    } finally {
      if (mounted) setState(() => _busy = false);
      done.complete();
      if (identical(_operation, done.future)) _operation = null;
    }
  }

  Future<void> _load() async {
    final epoch = _epoch;
    final organization = _organization;
    final choices = await _client.organizations();
    if (!_current(epoch, organization)) return;
    if (organization == null) {
      setState(() => _choices = choices);
      return;
    }
    final company = await organization.overview();
    if (!_current(epoch, organization)) return;
    native.OrganizationMemberPageView? members;
    native.OrganizationInvitationPageView? invitations;
    if (company.member.role == native.OrganizationRoleView.admin) {
      members = await organization.members();
      if (!_current(epoch, organization)) return;
      invitations = await organization.invitations();
      if (!_current(epoch, organization)) return;
    }
    setState(() {
      _choices = choices;
      _company = company;
      _members = members;
      _invitations = invitations;
    });
  }

  Future<void> _select(String id) async {
    final choices = _choices.where((choice) => choice.companyId == id).toList();
    final epoch = ++_epoch;
    setState(() {
      _retireSelection();
      _completed = false;
    });
    if (id.isEmpty) {
      await _load();
      return;
    }
    if (choices.length != 1) {
      setState(() => _error = 'authorization_denied');
      return;
    }
    final choice = choices.single;
    final owner = await _client.selectOrganization(
      company: choice.companyId,
      membershipRevision: choice.member.revision,
    );
    if (!_current(epoch)) {
      owner.close();
      owner.dispose();
      return;
    }
    setState(() {
      _organization = owner;
      _selectedId = choice.companyId;
    });
    await _load();
  }

  Future<void> _screens(native.OrganizationMemberView member) async {
    final organization = _organization;
    final epoch = _epoch;
    if (organization == null || !_current(epoch, organization)) return;
    await Navigator.of(context).push<void>(
      MaterialPageRoute<void>(
        builder: (_) => ScreenTargets(
          client: _client,
          userId: member.userId,
          organization: organization,
        ),
      ),
    );
    if (_current(epoch, organization)) await _run(_load);
  }

  Future<void> _execute() async {
    final command = _command!;
    final epoch = _epoch;
    await command.execute();
    if (!_current(epoch) || !identical(command, _command)) return;
    command.dispose();
    _command = null;
    setState(() {
      _completed = true;
      _emailEpoch++;
    });
    _email.clear();
    _name.clear();
    // A failed refresh does not undo the acknowledged mutation.
    await _load();
  }

  Future<void> _submit(Future<native.CompanyCommand> Function() prepare) =>
      _run(() async {
        if (_command != null) return;
        final epoch = _epoch;
        _completed = false;
        _reviewedUnknown = false;
        final command = await prepare();
        if (!_current(epoch)) {
          command.dispose();
          return;
        }
        _command = command;
        await _execute();
      });
  Future<void> _review() async {
    if (_organization != null && !_organization!.isCurrent()) {
      _epoch++;
      _retireSelection();
    }
    await _load();
    if (!_samePersonal || _leaving) return;
    _command?.dispose();
    _command = null;
    setState(() => _reviewedUnknown = true);
  }

  Future<void> _change(
    native.OrganizationMemberView member,
    native.OrganizationRoleView? role,
  ) async {
    final organization = _organization;
    final epoch = _epoch;
    if (organization == null ||
        !_current(epoch, organization) ||
        _confirmation != null) {
      return;
    }
    final l = AppLocalizations.of(context)!;
    final navigator = Navigator.of(context, rootNavigator: true);
    final confirmation = DialogRoute<bool>(
      context: context,
      themes: InheritedTheme.capture(from: context, to: navigator.context),
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
    _confirmation = confirmation;
    final confirmed = await navigator.push<bool>(confirmation);
    if (identical(_confirmation, confirmation)) _confirmation = null;
    if (confirmed != true || !_current(epoch, organization)) return;
    await _submit(
      () => organization.changeMember(
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
    final organization = _organization;
    final enabled =
        !_busy &&
        !_paused &&
        !_leaving &&
        _command == null &&
        (organization == null || organization.isCurrent());
    String role(native.OrganizationRoleView value) =>
        value == native.OrganizationRoleView.admin
        ? l.companyAdmin
        : l.companyUser;
    return PopScope<void>(
      canPop: _canPop,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) unawaited(_leave());
      },
      child: Scaffold(
        appBar: AppBar(title: Text(l.companyTitle)),
        body: ListView(
          padding: const EdgeInsets.all(OpenNetworkSpacing.six),
          children: [
            Text(l.companyPrivacy),
            DropdownButtonFormField<String>(
              key: ValueKey('organization-selector-$_epoch'),
              initialValue: _selectedId ?? '',
              decoration: InputDecoration(labelText: l.companySpace),
              isExpanded: true,
              items: [
                DropdownMenuItem(value: '', child: Text(l.companyPersonal)),
                for (final choice in _choices)
                  DropdownMenuItem(
                    value: choice.companyId,
                    child: Text(
                      '${choice.name} · ${role(choice.member.role)} · ${choice.companyId}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
              ],
              onChanged: enabled
                  ? (value) {
                      if (value != null) unawaited(_run(() => _select(value)));
                    }
                  : null,
            ),
            if (organization == null) Text(l.companyPersonalHelp),
            const SizedBox(height: OpenNetworkSpacing.four),
            TextField(
              key: const Key('company-name'),
              controller: _name,
              enabled: enabled,
              decoration: InputDecoration(labelText: l.companyName),
            ),
            FilledButton(
              key: const Key('company-create'),
              onPressed: enabled
                  ? () => _submit(
                      () => _client.createOrganization(name: _name.text.trim()),
                    )
                  : null,
              child: Text(l.companyCreate),
            ),
            Text(l.companyCreateHelp),
            if (_reviewedUnknown)
              Semantics(
                liveRegion: true,
                child: Text(l.companyReviewedUnknown),
              ),
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
                  'authentication_failed' || 'device_revoked' => l.authError,
                  'credential_revalidation_required' ||
                  'cancelled' => l.companyContextClosed,
                  'organization_context_unavailable' => l.companyServerRequired,
                  _ => l.genericError,
                }, key: const Key('company-error')),
              ),
            if (_command != null && !_leaving) ...[
              Text(l.companyPending),
              FilledButton(
                onPressed:
                    _busy || _paused || _leaving || !_command!.isCurrent()
                    ? null
                    : () => _run(_execute),
                child: Text(l.companyRetry),
              ),
              TextButton(
                onPressed: _busy || _paused || _leaving
                    ? null
                    : () => _run(_review),
                child: Text(l.companyReview),
              ),
            ],
            TextButton(
              onPressed: _busy || _paused || _leaving
                  ? null
                  : () => _run(_load),
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
                              if (native.screenSupported() &&
                                  company?.companyId ==
                                      _client.snapshot().tenantId)
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
                          final epoch = _epoch;
                          final page = await organization!.members(after: next);
                          if (_current(epoch, organization)) {
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
                        () => organization!.invite(
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
                                    () => organization!.revoke(
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
                          final epoch = _epoch;
                          final page = await organization!.invitations(
                            after: next,
                          );
                          if (_current(epoch, organization)) {
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
      ),
    );
  }
}
