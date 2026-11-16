import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

export function MetricCard({
  icon,
  label,
  value,
  detail,
}: {
  icon: IconName;
  label: string;
  value: ReactNode;
  detail: string;
}) {
  return (
    <div className="metric">
      <span>
        {label} <Icon name={icon} />
      </span>
      <strong>
        {value}
        <small>{detail}</small>
      </strong>
    </div>
  );
}
