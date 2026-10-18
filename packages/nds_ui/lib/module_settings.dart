import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'l10n/app_localizations.dart';

String modulePermissionLabel(AppLocalizations l, String value) =>
    switch (value) {
      'read_only' => l.permissionReadOnly,
      'credential_store' => l.permissionCredentialStore,
      'process_execution' => l.permissionProcessExecution,
      'local_network' => l.permissionLocalNetwork,
      'user_service' => l.permissionUserService,
      'clipboard' => l.permissionClipboard,
      _ => value,
    };

String moduleReadinessLabel(AppLocalizations l, String value) =>
    switch (value) {
      'disabled' => l.moduleDisabled,
      'unsupported' => l.providerUnavailable,
      'not_installed' => l.moduleNotInstalled,
      'permission_required' => l.modulePermissionRequired,
      'conflicted' => l.moduleConfigurationConflict,
      'ready' => l.reachable,
      'unavailable' => l.providerUnavailable,
      'failed' => l.providerFailed,
      _ => l.notChecked,
    };

String moduleFailureLabel(AppLocalizations l, String value) => switch (value) {
  'module_disabled' || 'ModuleDisabled' => l.moduleDisabled,
  'module_permission_required' ||
  'ModulePermissionRequired' => l.modulePermissionRequired,
  'module_configuration_changed' => l.moduleSettingsChanged,
  'module_configuration_conflict' ||
  'ModuleConfigurationConflict' => l.moduleConfigurationConflict,
  'module_unsupported' || 'unsupported_platform' => l.providerUnavailable,
  'module_not_installed' => l.moduleNotInstalled,
  'credential_revalidation_required' ||
  'device_revoked' => l.revalidationRequired,
  'provider_unavailable' => l.providerUnavailable,
  'operation_busy' ||
  'busy' ||
  'vault_busy' ||
  'online_verification_required' => l.rateError,
  'operation_timeout' => l.moduleNetworkError,
  'invalid_response' || 'invalid_module_contract' => l.moduleResponseError,
  'operation_cancelled' || 'cancelled' => l.moduleCancelled,
  'invalid_input' => l.moduleInputError,
  _ => l.providerFailed,
};

/// Presentation draft only. Rust owns permission validation, CAS and admission.
class ModuleSettings extends StatefulWidget {
  const ModuleSettings({
    super.key,
    required this.client,
    required this.module,
    required this.onSaved,
  });
  final native.NdsClient client;
  final native.DeviceModuleView module;
  final Future<void> Function() onSaved;
  @override
  State<ModuleSettings> createState() => _ModuleSettingsState();
}

class _ModuleSettingsState extends State<ModuleSettings> {
  late native.DeviceModuleView _basis;
  late bool _enabled;
  late Set<String> _permissions;
  bool _saving = false;
  bool _saved = false;
  String? _error;

  void _restore() {
    _basis = widget.module;
    _enabled = widget.module.enabled;
    _permissions = widget.module.permissions.toSet();
  }

  @override
  void initState() {
    super.initState();
    _restore();
  }

  bool _sameSetting(native.DeviceModuleView a, native.DeviceModuleView b) =>
      a.enabled == b.enabled &&
      a.conflicted == b.conflicted &&
      setEquals(a.permissions.toSet(), b.permissions.toSet());
  bool get _changed =>
      _enabled != _basis.enabled ||
      !setEquals(_permissions, _basis.permissions.toSet());
  bool get _stale =>
      (_basis.revision != widget.module.revision ||
          _basis.pendingId != widget.module.pendingId ||
          _basis.conflicted != widget.module.conflicted) &&
      !_sameSetting(_basis, widget.module);
  @override
  void didUpdateWidget(covariant ModuleSettings oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.module.id != widget.module.id || !_changed || _saved) {
      _restore();
    } else if (_sameSetting(_basis, widget.module)) {
      // Acknowledgement of the same displayed values preserves the user's draft.
      _basis = widget.module;
    }
  }

  Future<void> _save() async {
    if (_saving) return;
    final client = widget.client;
    final module = _basis;
    setState(() {
      _saving = true;
      _error = null;
      _saved = false;
    });
    try {
      await client.configureModule(
        module: module.id,
        enabled: _enabled,
        permissions: _permissions.toList()..sort(),
        revision: module.revision,
        pendingId: module.pendingId,
      );
      if (mounted && identical(client, widget.client)) {
        setState(() => _saved = true);
      }
      await widget.onSaved();
    } catch (error) {
      var code = 'unexpected';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      }
      if (mounted && identical(client, widget.client)) {
        setState(() => _error = code);
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final module = widget.module;
    final changed = _changed;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(moduleReadinessLabel(l, module.readiness)),
        Text(switch (module.installed) {
          true => l.moduleInstalled,
          false => l.moduleNotInstalled,
          null => l.moduleInstallationUnknown,
        }),
        SwitchListTile(
          key: const Key('module-enabled'),
          title: Text(l.moduleEnabled),
          value: _enabled,
          onChanged: _saving || module.conflicted || !module.supported
              ? null
              : (value) => setState(() {
                  _enabled = value;
                  _saved = false;
                }),
        ),
        if (module.requiredPermissions.isNotEmpty) Text(l.modulePermissions),
        for (final permission in module.requiredPermissions)
          CheckboxListTile(
            key: Key('module-permission-$permission'),
            title: Text(modulePermissionLabel(l, permission)),
            value: _permissions.contains(permission),
            onChanged: _saving || module.conflicted
                ? null
                : (value) => setState(() {
                    if (value == true) {
                      _permissions.add(permission);
                    } else {
                      _permissions.remove(permission);
                    }
                    _saved = false;
                  }),
          ),
        FilledButton(
          key: const Key('module-settings-save'),
          onPressed: _saving || _stale || !changed || module.conflicted
              ? null
              : _save,
          child: Text(l.moduleSave),
        ),
        if (_stale) ...[
          Text(l.moduleSettingsChanged),
          TextButton(
            onPressed: _saving
                ? null
                : () => setState(() {
                    _restore();
                    _saved = false;
                  }),
            child: Text(l.moduleReloadDraft),
          ),
        ],
        if (_saving) const LinearProgressIndicator(),
        if (_saved)
          Text(l.moduleSaved, key: const Key('module-settings-saved')),
        if (module.pendingId != null)
          Text(l.vaultCounts(1, module.conflicted ? 1 : 0)),
        if (_error case final error?)
          Text(
            moduleFailureLabel(l, error),
            key: const Key('module-settings-error'),
          ),
      ],
    );
  }
}
