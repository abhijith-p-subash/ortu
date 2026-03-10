export interface ClipboardItem {
    id: number;
    content_type: string;
    raw_content: string;
    category: string | null;
    groups: string[];
    is_permanent: boolean;
    created_at: string;
}

export interface Snippet {
    id: number;
    name: string;
    body: string;
    created_at: string;
    updated_at: string;
}
