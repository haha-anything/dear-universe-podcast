-- 002_queue_order.sql — 队列支持手动排序
--
-- 001 里队列的顺序就是自增 id 的顺序，这对「追加」够用，但没法重排。
-- 拖动排序如果靠改 id 实现，CASCADE 会把关联的队列项连带删掉（id 是主键），
-- 所以单独加一个 sort_order，重排只动这一列。

ALTER TABLE queue_item ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

-- 老数据按原 id 顺序铺一遍，保证加了列之后顺序不乱
UPDATE queue_item SET sort_order = id WHERE sort_order = 0;

CREATE INDEX IF NOT EXISTS idx_queue_order ON queue_item(sort_order, id);
