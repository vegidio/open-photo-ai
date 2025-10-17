import { Sidebar as FlowbiteSidebar, SidebarItem, SidebarItemGroup } from 'flowbite-react';

type SidebarProps = {
    className?: string;
};

export const Sidebar = ({ className }: SidebarProps) => {
    return (
        <FlowbiteSidebar className={`${className}`}>
            <SidebarItemGroup>
                <SidebarItem>Name 1</SidebarItem>
                <SidebarItem>Name 2</SidebarItem>
            </SidebarItemGroup>
        </FlowbiteSidebar>
    );
};
