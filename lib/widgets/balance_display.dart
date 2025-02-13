import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:fluttermint/data/balance.dart';

final balanceStreamProvider = StreamProvider.autoDispose<String?>((ref) {
  Stream<String?> getBalance() async* {
    var shouldPoll = true;
    while (shouldPoll) {
      try {
        await Future.delayed(const Duration(seconds: 5));
        debugPrint("polling balance");

        await ref.read(balanceProvider.notifier).refreshBalance();
        yield "good";
      } catch (e) {
        yield null;
      }
    }
  }

  return getBalance();
});

class BalanceDisplay extends ConsumerWidget {
  const BalanceDisplay({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final balance = ref.watch(balanceProvider);
    final balanceNotifier = ref.watch(balanceProvider.notifier);
    final balanceStreamWatcher = ref.watch(balanceStreamProvider);

    ref.listen<Balance?>(balanceProvider, (_, balance) {
      if (balance != null) {
        debugPrint(balance.toString());
      } else {
        debugPrint("balance is null?");
      }
    });

    return GestureDetector(
      onTap: () => {balanceNotifier.switchDenom()},
      child: Column(
        children: [
          balanceStreamWatcher.when(
              data: (_) => Text(balance != null ? balance.prettyPrint() : "???",
                  style: Theme.of(context).textTheme.headline1),
              loading: () =>
                  Text("~", style: Theme.of(context).textTheme.headline1),
              error: (err, _) => Text(err.toString())),
          balanceStreamWatcher.when(
              data: (_) => Text(
                  balance != null
                      ? balance.denomination.toReadableString()
                      : "NULL",
                  style: Theme.of(context).textTheme.headline2),
              loading: () =>
                  Text("-", style: Theme.of(context).textTheme.headline1),
              error: (err, _) => Text(err.toString()))
        ],
      ),
    );
  }
}
