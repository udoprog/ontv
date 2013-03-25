from distutils.core import setup

VERSION = '0.2.0'


def read_requirements():
    with open("requirements.txt") as f:
        for line in f:
            line = line.strip()

            if not line:
                continue

            yield line


setup(
    name='tvdb',
    version=VERSION,
    description="Your personal tv series manager",
    long_description="""
    tvdb is a tool that helps you keep track of what you are looking at right
    now.

    It is based of data from thetvdb.com which requires an account to access.
    """,
    author='John-John Tedro',
    author_email='johnjohn.tedro@gmail.com',
    url='http://github.com/udoprog/tvdb',
    license='GPLv3',
    packages=[
        'tvdb',
        'tvdb.action'
    ],
    scripts=['bin/tvdb'],
    requires=list(read_requirements()),
)
