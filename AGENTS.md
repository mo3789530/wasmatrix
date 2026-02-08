このプロジェクトは vibe coding です。

ルール：
- Feature-Sliced Design を使います
- src/features/{featureName} 単位で実装してください
- controller / service / repo の責務を分けてください
- controller は薄く、ロジックは service に書いてください
- DB や外部APIアクセスは repo に閉じ込めてください
- 既存構造を壊さず、必要最小限の変更にしてください
- 実装時にはテストとビルドが通ることを確認してください
- タスクを実行時には/docsになにを変更したか残すようにしてください
- .kiroにタスクのステータスを更新してください

不明点があれば、勝手に補完せず質問してください。
