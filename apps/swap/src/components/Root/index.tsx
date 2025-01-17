import type { ReactNode } from "react";

type Props = {
  children: ReactNode;
};

export const Root = ({ children }: Props) => {
  return (
    <html>
      <body>
        <div>
          <h1>Hello World!</h1>
          {children}
        </div>
      </body>
    </html>
  );
};
