import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';

class RepositoriesScreen extends StatefulWidget {
  const RepositoriesScreen({super.key, required this.client});
  final native.NdsClient client;

  @override
  State<RepositoriesScreen> createState() => _RepositoriesScreenState();
}

class _RepositoriesScreenState extends State<RepositoriesScreen> {
  List<native.GithubCredentialReference> _credentials = [];
  native.GithubCredentialReference? _selected;
  native.GithubSourceView? _source;
  native.GithubRepositoryPageView? _page;
  native.GithubSavedRepositoryPageView? _savedPage;
  bool _savedMode = true;
  String _nextCredential = '';
  bool _moreCredentials = false;
  bool _credentialsLoaded = false;
  bool _busy = false;
  native.GithubCatalogError? _error;
  int _actionEpoch = 0;
  bool _wasPaused = false;
  bool get _paused => !ApplicationActivityScope.read(context).value;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !_paused) unawaited(_loadSaved());
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    ApplicationActivityScope.of(context);
    if (_paused) {
      _actionEpoch++;
      _stopCatalog();
      _clearView();
    } else if (_wasPaused) {
      final client = widget.client;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && widget.client == client && !_paused) {
          unawaited(_savedMode ? _loadSaved() : _loadCredentials());
        }
      });
    }
    _wasPaused = _paused;
  }

  void _clearView() {
    _page = null;
    _savedPage = null;
    _selected = null;
    _source = null;
    _credentials = [];
    _nextCredential = '';
    _moreCredentials = false;
    _credentialsLoaded = false;
    _error = null;
    _busy = false;
  }

  @override
  void didUpdateWidget(covariant RepositoriesScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.client != widget.client) {
      _actionEpoch++;
      _stopCatalog(oldWidget.client);
      _clearView();
      _savedMode = true;
      final client = widget.client;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && widget.client == client && !_paused) {
          unawaited(_loadSaved());
        }
      });
    }
  }

  @override
  void dispose() {
    _actionEpoch++;
    _stopCatalog();
    super.dispose();
  }

  Future<void> _run(Future<void> Function(int epoch) action) async {
    if (_busy || _paused) return;
    final client = widget.client;
    final epoch = ++_actionEpoch;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await action(epoch);
    } on native.GithubCatalogError catch (error) {
      if (error == native.GithubCatalogError.busy &&
          mounted &&
          epoch == _actionEpoch &&
          !_paused) {
        // A receipt may advance source CAS metadata while this screen is open.
        // Refresh the visible choice before an explicit retry; never retry a write.
        try {
          final source = await widget.client.githubCatalogSource();
          if (mounted && epoch == _actionEpoch && !_paused) {
            setState(() {
              _source = source;
              _selected = source.credential;
            });
          }
        } catch (_) {
          if (mounted && epoch == _actionEpoch && !_paused) {
            setState(() {
              _source = null;
              _selected = null;
            });
          }
        }
      }
      if (mounted && epoch == _actionEpoch && !_paused) {
        setState(() {
          _error = error;
          // Keep the last accepted page so its next-page action can resume
          // the Rust-owned remainder after backpressure or a bounded wait.
          if (error != native.GithubCatalogError.busy &&
              error != native.GithubCatalogError.timeout &&
              error != native.GithubCatalogError.capacityExhausted) {
            _page = null;
            _savedPage = null;
          }
        });
      }
    } catch (_) {
      native.recordClientFailure(kind: native.FailureKind.nativeBridge);
      if (mounted && epoch == _actionEpoch && !_paused) {
        setState(() {
          _error = native.GithubCatalogError.unavailable;
          _page = null;
          _savedPage = null;
        });
      }
    } finally {
      if (mounted && widget.client == client && epoch == _actionEpoch) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _loadCredentials({bool next = false}) => _run((epoch) async {
    final source = await widget.client.githubCatalogSource();
    final result = await widget.client.githubCatalogCredentials(
      after: next ? _nextCredential : '',
    );
    if (!mounted || epoch != _actionEpoch || _paused) return;
    setState(() {
      _source = source;
      final selected = source.credential;
      _selected = selected == null
          ? null
          : result.items.firstWhere(
              (record) => _credentialKey(record) == _credentialKey(selected),
              orElse: () => selected,
            );
      _credentials = result.items;
      _credentialsLoaded = true;
      _nextCredential = result.nextAfter;
      _moreCredentials = result.hasMore;
    });
  });

  static String _credentialKey(native.GithubCredentialReference record) =>
      jsonEncode([record.recordRef, record.revision, record.pendingId]);

  Future<void> _select(native.GithubCredentialReference? record) =>
      _run((epoch) async {
        final source = _source;
        if (source == null || source.conflicted) return;
        // Dropdown form fields update optimistically. Recreate the field before
        // awaiting persistence so a refusal cannot display an unsaved choice.
        setState(() {
          _selected = null;
          _page = null;
        });
        if (record == null) {
          await widget.client.clearGithubCatalogSource(
            expectedRevision: source.revision,
            expectedPending: source.pendingId,
          );
        } else {
          await widget.client.selectGithubCredential(
            recordRef: record.recordRef,
            revision: record.revision,
            pendingId: record.pendingId,
            expectedSourceRevision: source.revision,
            expectedSourcePending: source.pendingId,
          );
        }
        if (!mounted || epoch != _actionEpoch || _paused) return;
        setState(() {
          _page = null;
          _selected = record;
          _source = null;
        });
        final current = await widget.client.githubCatalogSource();
        if (!mounted || epoch != _actionEpoch || _paused) return;
        setState(() {
          _source = current;
          _selected = current.credential;
        });
      });

  void _cancel() {
    _actionEpoch++;
    _stopCatalog();
    setState(() {
      _page = null;
      _savedPage = null;
      _busy = false;
      _error = native.GithubCatalogError.cancelled;
    });
  }

  void _stopCatalog([native.NdsClient? client]) {
    try {
      (client ?? widget.client).cancelGithubCatalog();
    } catch (_) {
      // The session owner may already have disposed this opaque client.
    }
  }

  Future<void> _loadPage({bool next = false}) => _run((epoch) async {
    if (!next) setState(() => _page = null);
    final page = await widget.client.githubRepositoryPage(nextPage: next);
    if (!mounted || epoch != _actionEpoch || _paused) return;
    setState(() => _page = page);
  });

  Future<void> _loadSaved({bool next = false}) => _run((epoch) async {
    if (!next) setState(() => _savedPage = null);
    final page = await widget.client.savedGithubRepositoryPage(nextPage: next);
    if (!mounted || epoch != _actionEpoch || _paused) return;
    setState(() => _savedPage = page);
  });

  void _mode(bool saved) {
    setState(() {
      _savedMode = saved;
      _error = null;
    });
    if (saved) {
      unawaited(_loadSaved());
    } else if (!_credentialsLoaded) {
      unawaited(_loadCredentials());
    }
  }

  String _errorText(AppLocalizations l, native.GithubCatalogError error) =>
      switch (error) {
        native.GithubCatalogError.moduleDisabled => l.moduleDisabled,
        native.GithubCatalogError.modulePermissionRequired =>
          l.modulePermissionRequired,
        native.GithubCatalogError.moduleConfigurationConflict =>
          l.moduleConfigurationConflict,
        native.GithubCatalogError.authorizationRequired => l.catalogSignIn,
        native.GithubCatalogError.credentialChanged =>
          l.catalogCredentialChanged,
        native.GithubCatalogError.busy => l.operationBusy,
        native.GithubCatalogError.cancelled => l.catalogCancelled,
        native.GithubCatalogError.timeout => l.catalogTimeout,
        native.GithubCatalogError.invalidInput =>
          _savedMode ? l.catalogReadSavedAgain : l.catalogChooseCredential,
        native.GithubCatalogError.invalidResponse => l.catalogInvalidResponse,
        native.GithubCatalogError.providerFailed => l.catalogProviderFailed,
        native.GithubCatalogError.unavailable => l.catalogUnavailable,
        native.GithubCatalogError.capacityExhausted => l.catalogCapacityReached,
      };

  String _observed(AppLocalizations l, String milliseconds) {
    final value = int.tryParse(milliseconds);
    if (value == null) return l.catalogObservationUnknown;
    try {
      return l.catalogObserved(
        DateFormat.yMd(Localizations.localeOf(context).toLanguageTag())
            .add_Hms()
            .format(DateTime.fromMillisecondsSinceEpoch(value).toLocal()),
      );
    } catch (_) {
      return l.catalogObservationUnknown;
    }
  }

  String _observation(AppLocalizations l, native.GithubObservationView value) =>
      [
        _observed(l, value.observedAtMs),
        if (value.pending) l.vaultRecordPending,
        if (value.conflicted) l.vaultConflict,
      ].join(' · ');

  Widget _repositoryTile(
    AppLocalizations l,
    native.GithubRepositorySummaryView repository, [
    native.GithubObservationView? observation,
  ]) => ListTile(
    key: ValueKey('catalog-repository-${repository.id}'),
    contentPadding: EdgeInsets.zero,
    title: Text(repository.name),
    subtitle: Text(
      [
        switch (repository.visibility) {
          native.GithubRepositoryVisibilityView.public => l.catalogPublic,
          native.GithubRepositoryVisibilityView.private => l.catalogPrivate,
          native.GithubRepositoryVisibilityView.internal => l.catalogInternal,
        },
        if (repository.archived) l.catalogArchived,
        if (repository.defaultBranch != null) repository.defaultBranch!,
        if (observation != null) _observation(l, observation),
      ].join(' · '),
    ),
  );

  Widget _ownerCard(
    AppLocalizations l,
    String ownerId,
    native.GithubOwnerView? owner,
    List<Widget> repositories, [
    native.GithubObservationView? observation,
  ]) => Card(
    key: ValueKey('catalog-owner-$ownerId'),
    child: Padding(
      padding: const EdgeInsets.all(OpenNetworkSpacing.four),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            owner?.login ?? l.catalogMissingOwner(ownerId),
            style: Theme.of(context).textTheme.titleMedium,
          ),
          if (owner != null)
            Text(
              owner.kind == native.GithubOwnerKindView.organization
                  ? l.catalogOrganization
                  : l.catalogPersonal,
            ),
          if (observation != null) Text(_observation(l, observation)),
          ...repositories,
        ],
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final page = _page;
    final choices = [..._credentials];
    if (_selected != null &&
        !choices.any(
          (item) => _credentialKey(item) == _credentialKey(_selected!),
        )) {
      choices.insert(0, _selected!);
    }
    return Scaffold(
      appBar: AppBar(title: Text(l.catalogTitle)),
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(OpenNetworkSpacing.six),
          children: [
            Wrap(
              spacing: OpenNetworkSpacing.two,
              runSpacing: OpenNetworkSpacing.two,
              children: [
                ChoiceChip(
                  label: Text(l.catalogSaved),
                  selected: _savedMode,
                  onSelected: _busy || _paused ? null : (_) => _mode(true),
                ),
                ChoiceChip(
                  key: const Key('catalog-mode-provider'),
                  label: Text(l.catalogProvider),
                  selected: !_savedMode,
                  onSelected: _busy || _paused ? null : (_) => _mode(false),
                ),
              ],
            ),
            const SizedBox(height: OpenNetworkSpacing.four),
            if (_busy) const LinearProgressIndicator(),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.symmetric(
                  vertical: OpenNetworkSpacing.four,
                ),
                child: Text(_errorText(l, _error!)),
              ),
            if (_paused) Text(l.catalogPaused),
            if (_savedMode) ...[
              Text(l.catalogSavedHelp),
              Wrap(
                spacing: OpenNetworkSpacing.two,
                children: [
                  FilledButton(
                    key: const Key('catalog-saved-read'),
                    onPressed: _busy || _paused ? null : _loadSaved,
                    child: Text(l.catalogReadSaved),
                  ),
                  if (_savedPage?.hasMore == true)
                    OutlinedButton(
                      key: const Key('catalog-saved-next'),
                      onPressed: _busy || _paused
                          ? null
                          : () => _loadSaved(next: true),
                      child: Text(l.catalogSavedNext),
                    ),
                  if (_busy)
                    TextButton(onPressed: _cancel, child: Text(l.actionCancel)),
                ],
              ),
              if (_savedPage != null) ...[
                if (_savedPage!.groups.isEmpty) Text(l.catalogEmptyPage),
                for (final group in _savedPage!.groups)
                  _ownerCard(l, group.ownerId, group.owner?.owner, [
                    for (final item in group.repositories)
                      _repositoryTile(l, item.repository, item.observation),
                  ], group.owner?.observation),
              ],
            ] else ...[
              Text(l.catalogHelp),
              Text(l.catalogSourceHelp),
              if (_source?.pendingId != null) Text(l.vaultRecordPending),
              if (_source?.conflicted == true) Text(l.vaultConflict),
              const SizedBox(height: OpenNetworkSpacing.four),
              DropdownButtonFormField<String>(
                key: ValueKey(
                  _selected == null ? null : _credentialKey(_selected!),
                ),
                initialValue: _selected == null
                    ? null
                    : _credentialKey(_selected!),
                isExpanded: true,
                decoration: InputDecoration(labelText: l.catalogCredential),
                items: choices
                    .map(
                      (record) => DropdownMenuItem(
                        value: _credentialKey(record),
                        child: Text(
                          record.title.isEmpty
                              ? record.recordRef
                              : record.title,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    )
                    .toList(),
                onChanged:
                    _busy || _paused || _source == null || _source!.conflicted
                    ? null
                    : (id) {
                        if (id != null) {
                          unawaited(
                            _select(
                              choices.firstWhere(
                                (r) => _credentialKey(r) == id,
                              ),
                            ),
                          );
                        }
                      },
              ),
              if (_selected?.username.isNotEmpty == true)
                Text(_selected!.username),
              Wrap(
                spacing: OpenNetworkSpacing.two,
                children: [
                  if (_source?.credential != null)
                    TextButton(
                      key: const Key('catalog-source-clear'),
                      onPressed: _busy || _paused || _source!.conflicted
                          ? null
                          : () => unawaited(_select(null)),
                      child: Text(l.catalogClearSource),
                    ),
                  TextButton(
                    onPressed: _busy || _paused ? null : _loadCredentials,
                    child: Text(l.catalogRefreshCredentials),
                  ),
                  if (_moreCredentials)
                    TextButton(
                      onPressed: _busy || _paused
                          ? null
                          : () => _loadCredentials(next: true),
                      child: Text(l.catalogMoreCredentials),
                    ),
                ],
              ),
              if (_credentialsLoaded &&
                  _credentials.isEmpty &&
                  !_busy &&
                  _error == null)
                Text(l.catalogNoCredentials),
              const SizedBox(height: OpenNetworkSpacing.four),
              Wrap(
                spacing: OpenNetworkSpacing.two,
                children: [
                  FilledButton(
                    key: const Key('catalog-read'),
                    onPressed: _busy || _paused || _selected == null
                        ? null
                        : _loadPage,
                    child: Text(l.catalogRead),
                  ),
                  if (page?.continuation ==
                      native.GithubCatalogContinuationView.more)
                    OutlinedButton(
                      key: const Key('catalog-next'),
                      onPressed: _busy || _paused
                          ? null
                          : () => _loadPage(next: true),
                      child: Text(l.catalogNextPage),
                    ),
                  if (_busy)
                    TextButton(onPressed: _cancel, child: Text(l.actionCancel)),
                ],
              ),
              if (page != null) ...[
                const SizedBox(height: OpenNetworkSpacing.six),
                Text(l.catalogPrincipal(page.principal.login)),
                Text(_observed(l, page.observedAtMs)),
                Text(l.catalogPageScope),
                if (page.groups.isEmpty) Text(l.catalogEmptyPage),
                for (final group in page.groups)
                  _ownerCard(l, group.owner.id, group.owner, [
                    for (final repository in group.repositories)
                      _repositoryTile(l, repository),
                  ]),
                Text(switch (page.continuation) {
                  native.GithubCatalogContinuationView.more => l.catalogHasMore,
                  native.GithubCatalogContinuationView.endOfAccessScope =>
                    l.catalogAccessEnd,
                  native.GithubCatalogContinuationView.limitReached =>
                    l.catalogLimitReached,
                }),
              ],
            ],
          ],
        ),
      ),
    );
  }
}
