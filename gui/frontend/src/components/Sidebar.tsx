import { Sidebar as FlowbiteSidebar, SidebarItem, SidebarItemGroup } from 'flowbite-react';
import type { CustomFlowbiteTheme } from 'flowbite-react/types';

type SidebarProps = {
    className?: string;
};

export const Sidebar = ({ className }: SidebarProps) => {
    const customTheme: CustomFlowbiteTheme['sidebar'] = {
        root: {
            base: 'bg-[#212121]',
            inner: 'bg-[#212121]',
        },
    };

    return (
        <FlowbiteSidebar theme={customTheme} className={`${className}`}>
            {/*<SidebarItemGroup>*/}
            {/*    <SidebarItem>Name 1</SidebarItem>*/}
            {/*    <SidebarItem>Name 2</SidebarItem>*/}
            {/*</SidebarItemGroup>*/}
        </FlowbiteSidebar>
    );
};
